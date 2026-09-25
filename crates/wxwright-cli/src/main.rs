//! wxwright CLI: the agent-facing entry point (PRD 6).
//! Exit codes align with the official verify CLI: 0 pass / 1 violations / 2 error.

mod output;

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use output::Out;
use wxwright_core::i18n::{set_lang, Lang};
use wxwright_core::img::ImageMode;
use wxwright_core::theme;
use wxwright_core::validator::Violation;
use wxwright_core::{clipboard, ConvertOptions, PipelineOutput};
use wxwright_mp::{Credentials, MpClient};

#[derive(ValueEnum, Clone, Copy)]
enum LangArg {
    En,
    ZhCn,
}

#[derive(Parser)]
#[command(
    name = "wxwright",
    version,
    about = "Markdown to WeChat MP compliant rich text - CLI / MCP engine for agents and humans",
    long_about = None
)]
struct Cli {
    /// Human-output language (en | zh-CN). Default: auto-detect.
    #[arg(long, value_enum)]
    lang: Option<LangArg>,
    /// Machine-readable JSON output (also the default when stdout is not a TTY).
    #[arg(long, global = true)]
    json: bool,
    /// Disable ANSI colors (also honors NO_COLOR).
    #[arg(long, global = true)]
    no_color: bool,
    /// Verbose diagnostics.
    #[arg(long, global = true)]
    verbose: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Markdown -> WeChat MP dialect HTML (self-contained; local images inlined).
    Convert {
        /// Input markdown file, or "-" for stdin.
        input: String,
        /// Theme id (minimal|techblue|magazine) or path to a theme TOML.
        #[arg(long)]
        theme: Option<String>,
        /// Output HTML file, or "-" for stdout.
        #[arg(long, default_value = "-")]
        out: String,
    },
    /// Validate markdown/HTML against the official editor spec.
    Validate {
        /// Input .md or .html file, or "-" for stdin.
        input: String,
        /// Exit with code 1 on blocking violations.
        #[arg(long)]
        strict: bool,
        /// Note the official puppeteer bridge (CI truth) availability.
        #[arg(long)]
        official_check: bool,
    },
    /// Auto-fix violations in an HTML file (normalizer report mode).
    Fix {
        input: String,
        #[arg(long)]
        out: String,
    },
    /// Full chain: convert -> images -> normalize -> validate -> clipboard.
    Copy {
        input: String,
        #[arg(long)]
        theme: Option<String>,
        /// Run the chain without writing the clipboard.
        #[arg(long)]
        dry_run: bool,
    },
    /// Image asset operations.
    Image {
        #[command(subcommand)]
        cmd: ImageCmd,
    },
    /// WeChat MP draft box (草稿箱) operations. Requires credentials.
    Draft {
        #[command(subcommand)]
        cmd: DraftCmd,
    },
    /// 群发 (mass send) a draft via freepublish. Explicit confirmation required.
    Publish {
        draft_id: String,
        #[arg(long)]
        yes: bool,
    },
    /// Theme scaffolding.
    Theme {
        #[command(subcommand)]
        cmd: ThemeCmd,
    },
    /// Environment health check.
    Doctor,
    /// MCP server operations.
    Mcp {
        #[command(subcommand)]
        cmd: McpCmd,
    },
    /// Print the agent hand-off card.
    AgentCard {
        #[arg(long)]
        md: bool,
        #[arg(long)]
        json: bool,
    },
    /// Store MP credentials in the OS keychain (AppID/Secret, PRD 5.6).
    Login {
        #[arg(long)]
        appid: String,
        #[arg(long)]
        secret: String,
        /// Store the secret in the config file instead of the keychain.
        #[arg(long)]
        no_keyring: bool,
    },
    /// Remove stored credentials.
    Logout,
    /// Quick performance measurement of the full in-process chain.
    Bench {
        /// Iterations.
        #[arg(long, default_value = "30")]
        iters: usize,
    },
}

#[derive(Subcommand)]
enum ImageCmd {
    /// Upload images to the permanent material library; prints mmbiz mapping.
    Upload {
        /// Image files to upload.
        paths: Vec<String>,
    },
}

#[derive(Subcommand)]
enum DraftCmd {
    /// Create a draft from markdown (images uploaded to mmbiz automatically).
    Create {
        #[arg(long)]
        file: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        digest: Option<String>,
        #[arg(long)]
        theme: Option<String>,
        /// thumb_media_id; defaults to the first uploaded article image.
        #[arg(long)]
        thumb_media_id: Option<String>,
    },
    /// Update an existing draft.
    Update {
        #[arg(long)]
        media_id: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        theme: Option<String>,
    },
    /// List drafts.
    List {
        #[arg(long, default_value = "0")]
        offset: u64,
        #[arg(long, default_value = "10")]
        count: u64,
    },
}

#[derive(Subcommand)]
enum ThemeCmd {
    List,
    /// Scaffold a new theme TOML.
    New {
        name: String,
    },
    /// Validate a theme file by rendering a sample document.
    Validate {
        path: String,
    },
}

#[derive(Subcommand)]
enum McpCmd {
    /// Start the stdio MCP server.
    Serve,
    /// Write MCP client configuration for the target app.
    Install {
        #[arg(long, value_enum)]
        target: McpTarget,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum McpTarget {
    Claude,
    Cursor,
    Vscode,
    Opencode,
}

fn detect_lang(arg: Option<LangArg>) -> Lang {
    if let Some(a) = arg {
        return match a {
            LangArg::En => Lang::En,
            LangArg::ZhCn => Lang::ZhCn,
        };
    }
    if let Ok(v) = std::env::var("WXWRIGHT_LANG") {
        if let Some(l) = Lang::from_code(&v) {
            return l;
        }
    }
    if let Some(code) = sys_locale::get_locale() {
        if let Some(l) = Lang::from_code(&code) {
            return l;
        }
    }
    Lang::En
}

fn main() {
    let cli = Cli::parse();
    set_lang(detect_lang(cli.lang));
    let out = Out::new(cli.json, cli.no_color);
    let code = match run(&cli, &out) {
        Ok(code) => code,
        Err(e) => {
            if out.json_mode {
                out.print_json(&serde_json::json!({ "ok": false, "error": e }));
            } else {
                out.err(&e);
            }
            2
        }
    };
    std::process::exit(code);
}

fn run(cli: &Cli, out: &Out) -> Result<i32, String> {
    match &cli.command {
        Commands::Convert {
            input,
            theme,
            out: out_path,
        } => cmd_convert(input, theme.as_deref(), out_path, out),
        Commands::Validate {
            input,
            strict,
            official_check,
        } => cmd_validate(input, *strict, *official_check, out),
        Commands::Fix {
            input,
            out: out_path,
        } => cmd_fix(input, out_path, out),
        Commands::Copy {
            input,
            theme,
            dry_run,
        } => cmd_copy(input, theme.as_deref(), *dry_run, out),
        Commands::Image { cmd } => match cmd {
            ImageCmd::Upload { paths } => cmd_image_upload(paths, out),
        },
        Commands::Draft { cmd } => cmd_draft(cmd, out),
        Commands::Publish { draft_id, yes } => cmd_publish(draft_id, *yes, out),
        Commands::Theme { cmd } => cmd_theme(cmd, out),
        Commands::Doctor => cmd_doctor(out),
        Commands::Mcp { cmd } => cmd_mcp(cmd, out),
        Commands::AgentCard { md, json } => cmd_agent_card(*md, *json, out),
        Commands::Login {
            appid,
            secret,
            no_keyring,
        } => cmd_login(appid, secret, *no_keyring, out),
        Commands::Logout => cmd_logout(out),
        Commands::Bench { iters } => cmd_bench(*iters, out),
    }
}

// ---------------------------------------------------------------- helpers ---

fn build_options(
    theme_name: Option<&str>,
    mode: ImageMode,
    base_dir: Option<PathBuf>,
) -> Result<ConvertOptions, String> {
    let t = theme::load_theme(theme_name.unwrap_or("minimal")).map_err(|e| e.to_string())?;
    let transport = match mode {
        ImageMode::Upload => {
            let creds = wxwright_mp::load_credentials().ok_or_else(|| {
                "image upload mode requires credentials; run `wxwright login`".to_string()
            })?;
            Some(Arc::new(MpClient::new(creds)) as Arc<dyn wxwright_core::img::ImageTransport>)
        }
        _ => None,
    };
    Ok(ConvertOptions {
        theme: t,
        image_mode: mode,
        base_dir,
        transport,
    })
}

fn read_stdin_if_dash(path: &str) -> Result<(String, Option<PathBuf>), String> {
    output::read_input(path).map_err(|e| format!("cannot read {}: {}", path, e))
}

fn violation_report<'a, I>(out: &Out, violations: I)
where
    I: IntoIterator<Item = &'a Violation>,
{
    for v in violations {
        let desc = wxwright_core::rules::rule_info(&v.rule_id)
            .map(|r| r.localized_desc())
            .unwrap_or_default();
        let sev = if v.is_block() { "BLOCK" } else { "WARN" };
        out.plain(&format!(
            "  {} {} {} - {}",
            if v.is_block() { "[ERR]" } else { "[WARN]" },
            v.rule_id,
            sev,
            desc
        ));
        out.plain(&format!("      node: {}", v.node));
        let _ = std::io::stdout().flush();
    }
}

fn report_payload(p: &PipelineOutput) -> serde_json::Value {
    serde_json::json!({
        "stats": p.stats,
        "images": p.images,
        "fixes": p.fixes,
        "violations": p.violations,
        "blocking_count": p.blocking_violations().len(),
    })
}

fn print_violations_human(out: &Out, p: &PipelineOutput) {
    let blocks = p.blocking_violations();
    let warns: Vec<_> = p.violations.iter().filter(|v| !v.is_block()).collect();
    if !blocks.is_empty() {
        out.err(&format!("{} blocking violation(s)", blocks.len()));
        violation_report(out, blocks.iter().copied());
    }
    if !warns.is_empty() {
        out.warn(&format!("{} warning(s)", warns.len()));
        violation_report(out, warns.iter().copied());
    }
    if !p.fixes.is_empty() {
        for f in &p.fixes {
            out.info(&format!("fixed [{}] x{}: {}", f.rule_id, f.count, f.detail));
        }
    }
}

// --------------------------------------------------------------- commands ---

fn cmd_convert(
    input: &str,
    theme_name: Option<&str>,
    out_path: &str,
    out: &Out,
) -> Result<i32, String> {
    let (md, base_dir) = read_stdin_if_dash(input)?;
    let opts = build_options(theme_name, ImageMode::Inline, base_dir)?;
    let result = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
    let html = wxwright_core::wrap_document(&result.html);
    let payload = serde_json::json!({
        "ok": true,
        "html": html,
        "stats": result.stats,
        "images": result.images,
        "fixes": result.fixes,
        "violations": result.violations,
        "blocking_count": result.blocking_violations().len(),
    });
    if out.json_mode {
        out.print_json(&payload);
    } else {
        out.ok(&format!(
            "converted ({} chars, {} images, {} violations)",
            result.stats.chars,
            result.images.len(),
            result.blocking_violations().len()
        ));
        print_violations_human(out, &result);
    }
    if out_path == "-" {
        if !out.json_mode {
            std::io::stdout()
                .write_all(html.as_bytes())
                .map_err(|e| e.to_string())?;
            println!();
        }
    } else {
        std::fs::write(out_path, &html).map_err(|e| format!("cannot write {}: {}", out_path, e))?;
        if !out.json_mode {
            out.ok(&format!("written to {}", out_path));
        }
    }
    Ok(0)
}

fn cmd_validate(input: &str, strict: bool, official_check: bool, out: &Out) -> Result<i32, String> {
    let (content, base_dir) = read_stdin_if_dash(input)?;
    let lower = input.to_ascii_lowercase();
    let is_html = if lower.ends_with(".md") || lower.ends_with(".markdown") {
        false
    } else if lower.ends_with(".html") || lower.ends_with(".htm") {
        true
    } else {
        // stdin / unknown extension: sniff
        content.trim_start().starts_with('<')
    };
    let html = if is_html {
        content
    } else {
        let opts = build_options(None, ImageMode::Inline, base_dir)?;
        wxwright_core::convert_markdown(&content, &opts)
            .map_err(|e| e.to_string())?
            .html
    };
    let violations = wxwright_core::validator::validate_html(&html);
    let blocking: Vec<_> = violations.iter().filter(|v| v.is_block()).collect();
    let payload = serde_json::json!({
        "ok": blocking.is_empty(),
        "compliant": blocking.is_empty(),
        "blocking_count": blocking.len(),
        "violations": violations,
    });
    if out.json_mode {
        out.print_json(&payload);
    } else if blocking.is_empty() {
        out.ok(&format!(
            "compliant ({} warning(s))",
            violations.len() - blocking.len()
        ));
        violation_report(
            out,
            &violations
                .iter()
                .filter(|v| !v.is_block())
                .cloned()
                .collect::<Vec<_>>(),
        );
    } else {
        out.err(&format!("{} blocking violation(s)", blocking.len()));
        violation_report(out, &violations);
    }

    if official_check {
        match which("node") {
            Some(node) => out.info(&format!(
                "official verify bridge: node found at {}. In CI, run the official check with a checkout of \
                 github.com/wechatjs/verify-article-structure-spec: `npm install && npm run check -- article.html` \
                 (exit 0 = pass). This command does not execute it.",
                node
            )),
            None => out.warn("official verify bridge: node not found on PATH; install Node.js to enable the CI truth gate"),
        }
    }

    if strict && !blocking.is_empty() {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn cmd_fix(input: &str, out_path: &str, out: &Out) -> Result<i32, String> {
    let (html, _) = read_stdin_if_dash(input)?;
    let result = wxwright_core::normalizer::normalize_html(&html, Default::default())
        .map_err(|e| e.to_string())?;
    std::fs::write(out_path, &result.html)
        .map_err(|e| format!("cannot write {}: {}", out_path, e))?;
    let remaining = wxwright_core::validator::validate_html(&result.html);
    let payload = serde_json::json!({
        "ok": true,
        "out": out_path,
        "fixes": result.fixes,
        "remaining_violations": remaining,
    });
    if out.json_mode {
        out.print_json(&payload);
    } else {
        out.ok(&format!(
            "fixed {} issue group(s) -> {}",
            result.fixes.len(),
            out_path
        ));
        for f in &result.fixes {
            out.info(&format!("[{}] x{}: {}", f.rule_id, f.count, f.detail));
        }
        let remaining_blocks: Vec<_> = remaining.iter().filter(|v| v.is_block()).collect();
        if !remaining_blocks.is_empty() {
            out.warn(&format!(
                "{} blocking issue(s) need manual attention",
                remaining_blocks.len()
            ));
            violation_report(out, remaining_blocks.iter().copied());
            return Ok(1);
        }
    }
    Ok(0)
}

fn cmd_copy(
    input: &str,
    theme_name: Option<&str>,
    dry_run: bool,
    out: &Out,
) -> Result<i32, String> {
    let (md, base_dir) = read_stdin_if_dash(input)?;
    let creds = wxwright_mp::load_credentials();
    let mode = if creds.is_some() {
        ImageMode::Upload
    } else {
        ImageMode::Keep
    };
    let mut opts = build_options(theme_name, mode, base_dir)?;
    if let Some(c) = &creds {
        opts.transport = Some(Arc::new(MpClient::new(c.clone())));
    }
    let result = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;

    // Blocking violations: never copy a non-compliant payload.
    let blocks = result.blocking_violations();
    if !blocks.is_empty() {
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": false, "copied": false,
                "reason": "blocking violations",
                "violations": result.violations,
                "fixes": result.fixes,
            }));
        } else {
            out.err("copy blocked: blocking violations (fix the markdown first)");
            print_violations_human(out, &result);
        }
        return Ok(1);
    }

    // I-03: clipboard payload must be mmbiz or https images; local/base64
    // images break on paste. Block and list them.
    let paste_hostile: Vec<_> = result
        .images
        .iter()
        .filter(|i| (i.inlined && !i.mmbiz) || i.source.starts_with("data:"))
        .map(|i| i.source.clone())
        .collect();
    if !paste_hostile.is_empty() && !dry_run {
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": false, "copied": false,
                "reason": "paste-hostile images: clipboard requires mmbiz or https URLs (I-03)",
                "local_images": paste_hostile,
                "hint": "run `wxwright login` to enable mmbiz upload, or use `convert --out file.html`",
            }));
        } else {
            out.err(
                "copy blocked: local/base64 images would break on paste (needs mmbiz or https)",
            );
            for s in &paste_hostile {
                out.plain(&format!("      image: {}", s));
            }
            out.info("run `wxwright login` to enable automatic mmbiz upload, or `convert --out file.html` for a file");
        }
        return Ok(1);
    }

    if dry_run {
        if out.json_mode {
            out.print_json(&serde_json::json!({ "ok": true, "dry_run": true, "report": report_payload(&result) }));
        } else {
            out.ok("dry run passed: chain is clean, clipboard not touched");
            print_violations_human(out, &result);
        }
        return Ok(0);
    }

    let copied = clipboard::copy_to_clipboard(&result.html).map_err(|e| e.to_string())?;
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": true, "copied": true, "html_flavor": copied.html_flavor,
            "report": report_payload(&result),
            "next_step": "paste into the MP editor (Ctrl+V)",
        }));
    } else {
        if copied.html_flavor {
            out.ok("rich text copied to clipboard");
        } else {
            out.warn("clipboard written as plain text (HTML flavor unavailable on this platform)");
        }
        out.info("open the WeChat MP editor and paste (Ctrl+V)");
        print_violations_human(out, &result);
    }
    Ok(0)
}

fn cmd_image_upload(paths: &[String], out: &Out) -> Result<i32, String> {
    let creds =
        wxwright_mp::load_credentials().ok_or("no credentials; run `wxwright login` first")?;
    let client = MpClient::new(creds);
    let mut results = Vec::new();
    for p in paths {
        let bytes = std::fs::read(p).map_err(|e| format!("cannot read {}: {}", p, e))?;
        let name = PathBuf::from(p)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "image.png".into());
        match client.upload_material(bytes, &name) {
            Ok((media_id, url)) => {
                results.push(serde_json::json!({ "source": p, "media_id": media_id, "url": url, "ok": true }));
            }
            Err(e) => {
                results
                    .push(serde_json::json!({ "source": p, "ok": false, "error": e.to_string() }));
            }
        }
    }
    let all_ok = results.iter().all(|r| r["ok"] == serde_json::json!(true));
    if out.json_mode {
        out.print_json(&serde_json::json!({ "ok": all_ok, "uploads": results }));
    } else {
        for r in &results {
            if r["ok"] == serde_json::json!(true) {
                out.ok(&format!(
                    "{} -> {}",
                    r["source"].as_str().unwrap_or(""),
                    r["url"].as_str().unwrap_or("")
                ));
            } else {
                out.err(&format!(
                    "{} -> {}",
                    r["source"].as_str().unwrap_or(""),
                    r["error"].as_str().unwrap_or("")
                ));
            }
        }
    }
    Ok(if all_ok { 0 } else { 1 })
}

fn cmd_draft(cmd: &DraftCmd, out: &Out) -> Result<i32, String> {
    let creds =
        wxwright_mp::load_credentials().ok_or("no credentials; run `wxwright login` first")?;
    let client = MpClient::new(creds);
    match cmd {
        DraftCmd::Create {
            file,
            title,
            author,
            digest,
            theme: theme_name,
            thumb_media_id,
        } => {
            let (md, base_dir) = read_stdin_if_dash(file)?;
            let mut opts = build_options(theme_name.as_deref(), ImageMode::Upload, base_dir)?;
            opts.transport = Some(Arc::new(MpClient::new(client_credentials(&client))));
            let result = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
            if !result.blocking_violations().is_empty() {
                out.err("draft blocked: blocking violations");
                if out.json_mode {
                    out.print_json(&report_payload(&result));
                } else {
                    print_violations_human(out, &result);
                }
                return Ok(1);
            }
            let thumb = match thumb_media_id {
                Some(t) => t.clone(),
                None => result
                    .images
                    .iter()
                    .find_map(|i| i.media_id.clone())
                    .ok_or("no image available for thumb_media_id; the API requires a cover - include at least one image or pass --thumb-media-id")?,
            };
            let article = wxwright_mp::DraftArticle {
                title: title.clone(),
                author: author.clone().unwrap_or_default(),
                digest: digest.clone().unwrap_or_default(),
                content_html: result.html.clone(),
                content_source_url: String::new(),
                thumb_media_id: thumb,
            };
            let media_id = client.draft_add(&article).map_err(|e| e.to_string())?;
            if out.json_mode {
                out.print_json(&serde_json::json!({ "ok": true, "draft_media_id": media_id, "report": report_payload(&result) }));
            } else {
                out.ok(&format!("draft created: {}", media_id));
                out.info("open MP -> 草稿箱 (Drafts) to review and publish");
            }
            Ok(0)
        }
        DraftCmd::Update {
            media_id,
            file,
            title,
            theme: theme_name,
        } => {
            let (md, base_dir) = read_stdin_if_dash(file)?;
            let opts = build_options(theme_name.as_deref(), ImageMode::Upload, base_dir)?;
            let result = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
            let article = wxwright_mp::DraftArticle {
                title: title.clone(),
                author: String::new(),
                digest: String::new(),
                content_html: result.html.clone(),
                content_source_url: String::new(),
                thumb_media_id: String::new(),
            };
            client
                .draft_update(media_id, 0, &article)
                .map_err(|e| e.to_string())?;
            if out.json_mode {
                out.print_json(&serde_json::json!({ "ok": true, "updated": media_id }));
            } else {
                out.ok(&format!("draft {} updated", media_id));
            }
            Ok(0)
        }
        DraftCmd::List { offset, count } => {
            let v = client
                .draft_list(*offset, *count)
                .map_err(|e| e.to_string())?;
            if out.json_mode {
                out.print_json(&v);
            } else {
                let total = v.get("total_count").and_then(|x| x.as_u64()).unwrap_or(0);
                out.ok(&format!(
                    "{} draft(s) (total {})",
                    v.get("item")
                        .and_then(|i| i.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0),
                    total
                ));
                if let Some(items) = v.get("item").and_then(|i| i.as_array()) {
                    for it in items {
                        let media_id = it.get("media_id").and_then(|x| x.as_str()).unwrap_or("");
                        let title = it
                            .get("content")
                            .and_then(|c| c.get("news_item"))
                            .and_then(|n| n.as_array())
                            .and_then(|a| a.first())
                            .and_then(|n| n.get("title"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("");
                        out.plain(&format!("  {}  {}", media_id, title));
                    }
                }
            }
            Ok(0)
        }
    }
}

fn client_credentials(_client: &MpClient) -> Credentials {
    wxwright_mp::load_credentials().unwrap_or(Credentials {
        appid: String::new(),
        secret: String::new(),
    })
}

fn cmd_publish(draft_id: &str, yes: bool, out: &Out) -> Result<i32, String> {
    if !yes {
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": false,
                "error": "publish requires explicit --yes; it performs 群发 (mass send) to followers"
            }));
        } else {
            out.err("publish performs 群发 (mass send) - rerun with --yes to confirm");
        }
        return Ok(2);
    }
    let creds =
        wxwright_mp::load_credentials().ok_or("no credentials; run `wxwright login` first")?;
    let client = MpClient::new(creds);
    let resp = client
        .freepublish_submit(draft_id)
        .map_err(|e| e.to_string())?;
    if out.json_mode {
        out.print_json(&resp);
    } else {
        out.ok(&format!("publish submitted for draft {}", draft_id));
        out.info("check the MP console -> 发表记录 (Publish Records) for status");
    }
    Ok(0)
}

fn cmd_theme(cmd: &ThemeCmd, out: &Out) -> Result<i32, String> {
    match cmd {
        ThemeCmd::List => {
            let mut themes = theme::builtin_themes().map_err(|e| e.to_string())?;
            let user = theme::list_user_themes();
            if out.json_mode {
                let list: Vec<serde_json::Value> = themes
                    .iter()
                    .map(|t| serde_json::to_value(&t.meta).unwrap_or_default())
                    .chain(user.iter().map(|t| {
                        let mut v = serde_json::to_value(&t.meta).unwrap_or_default();
                        v["user"] = serde_json::json!(true);
                        v
                    }))
                    .collect();
                out.print_json(&serde_json::json!({ "themes": list }));
            } else {
                for t in &themes {
                    out.plain(&format!(
                        "  {:<14} {} / {}  ({})",
                        t.meta.id, t.meta.name, t.meta.name_zh, t.meta.description_zh
                    ));
                }
                for t in &user {
                    out.plain(&format!(
                        "  {:<14} {} / {}  ({}) [user]",
                        t.meta.id, t.meta.name, t.meta.name_zh, t.meta.description_zh
                    ));
                }
                out.info("pass a path to a .toml file to use a custom theme; user themes live in the wxwright themes directory");
            }
            let _ = &mut themes;
            Ok(0)
        }
        ThemeCmd::New { name } => {
            let path = format!("{}.toml", name);
            let body = theme_scaffold(name);
            std::fs::write(&path, body).map_err(|e| format!("cannot write {}: {}", path, e))?;
            if out.json_mode {
                out.print_json(&serde_json::json!({ "ok": true, "path": path }));
            } else {
                out.ok(&format!("theme scaffold written to {}", path));
                out.info("edit [colors] and [block.*] overrides, then check with `wxwright theme validate`");
            }
            Ok(0)
        }
        ThemeCmd::Validate { path } => {
            let t = theme::load_theme(path).map_err(|e| e.to_string())?;
            let sample = "# Heading\n\nParagraph with **bold** and `code`.\n\n> quote\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
            let opts = ConvertOptions::new(t.clone());
            let result = wxwright_core::pipeline(sample, &opts).map_err(|e| e.to_string())?;
            let blocks = result.blocking_violations();
            if out.json_mode {
                out.print_json(&serde_json::json!({ "ok": blocks.is_empty(), "theme": t.meta, "violations": result.violations }));
            } else if blocks.is_empty() {
                out.ok(&format!(
                    "theme {} is valid and produces compliant output",
                    t.meta.id
                ));
            } else {
                out.err(&format!(
                    "theme {} produces non-compliant output",
                    t.meta.id
                ));
                violation_report(out, blocks.iter().copied());
            }
            Ok(if blocks.is_empty() { 0 } else { 1 })
        }
    }
}

fn theme_scaffold(name: &str) -> String {
    format!(
        r##"[meta]
id = "{name}"
name = "{Name}"
name_zh = "{Name}"
author = "your name"
license = "MIT"
description = "One-line description."
description_zh = "一句话描述。"
link_style = "footnote"   # footnote | inline
code_theme = "light"      # light | dark

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

# Optional style overrides per role (no font-family allowed, R-3.1).
[block.h2]
border-left = "4px solid {{accent}}"
padding-left = "10px"

[block.h2_leaf]
color = "{{accent}}"

[color_dark]
text = "#C9D1D9"
background = "#191919"
accent = "#5B8DEF"
"##,
        name = name,
        Name = // capitalize best-effort
            { let mut c = name.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } },
    )
}

fn which(exe: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(format!(
            "{}{}",
            exe,
            if cfg!(windows) { ".exe" } else { "" }
        ));
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn cmd_doctor(out: &Out) -> Result<i32, String> {
    let mut checks: Vec<serde_json::Value> = Vec::new();
    let mut push = |name: &str, status: &str, detail: serde_json::Value| {
        checks.push(serde_json::json!({ "check": name, "status": status, "detail": detail }));
    };

    push(
        "version",
        "ok",
        serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }),
    );

    let config_exists = wxwright_mp::config_path().exists();
    push(
        "config_file",
        if config_exists { "ok" } else { "warn" },
        serde_json::json!({
            "path": wxwright_mp::config_path().to_string_lossy(),
            "exists": config_exists,
        }),
    );

    match wxwright_mp::load_credentials() {
        Some(c) => push(
            "credentials",
            "ok",
            serde_json::json!({
                "appid": c.appid,
                "secret": wxwright_mp::mask(&c.secret),
            }),
        ),
        None => push(
            "credentials",
            "warn",
            serde_json::json!({
                "hint": "run `wxwright login --appid <id> --secret <secret>` to enable mmbiz upload and drafts",
            }),
        ),
    }

    let clipboard_ok = arboard_probe();
    push(
        "clipboard",
        if clipboard_ok { "ok" } else { "warn" },
        serde_json::json!({ "available": clipboard_ok }),
    );

    let themes = theme::builtin_themes().map(|t| t.len()).unwrap_or(0);
    push(
        "themes",
        if themes >= 3 { "ok" } else { "warn" },
        serde_json::json!({ "builtin_count": themes }),
    );

    let net = wxwright_mp::probe_reachable();
    push(
        "mp_network",
        if net { "ok" } else { "warn" },
        serde_json::json!({
            "reachable": net,
            "endpoint": "api.weixin.qq.com",
        }),
    );

    let comfy = wxwright_core::util::probe_tcp("127.0.0.1", 8188, 800);
    push(
        "comfyui",
        if comfy { "ok" } else { "warn" },
        serde_json::json!({
            "endpoint": "127.0.0.1:8188",
            "detected": comfy,
            "purpose": "local text-to-image / image-to-image (GUI AI painting)",
        }),
    );

    let node = which("node");
    push(
        "node_official_bridge",
        if node.is_some() { "ok" } else { "warn" },
        serde_json::json!({ "node": node, "purpose": "official verify-article-structure-spec CI gate" }),
    );

    let problems = checks.iter().any(|c| c["status"] == "err");
    if out.json_mode {
        out.print_json(&serde_json::json!({ "ok": !problems, "checks": checks }));
    } else {
        for c in &checks {
            let status = c["status"].as_str().unwrap_or("");
            let line = format!("{:<22} {}", c["check"].as_str().unwrap_or(""), c["detail"]);
            match status {
                "ok" => out.ok(&line),
                "warn" => out.warn(&line),
                _ => out.err(&line),
            }
        }
    }
    Ok(0)
}

fn arboard_probe() -> bool {
    // In headless contexts this fails; doctor treats it as a warning.
    std::thread::spawn(|| wxwright_core::clipboard::probe().is_ok())
        .join()
        .unwrap_or(false)
}

fn cmd_mcp(cmd: &McpCmd, out: &Out) -> Result<i32, String> {
    match cmd {
        McpCmd::Serve => {
            wxwright_mcp::run_stdio().map_err(|e| format!("mcp serve: {}", e))?;
            Ok(0)
        }
        McpCmd::Install { target } => {
            let target_str = match target {
                McpTarget::Claude => "claude",
                McpTarget::Cursor => "cursor",
                McpTarget::Vscode => "vscode",
                McpTarget::Opencode => "opencode",
            };
            let outcome = wxwright_mcp::install::install(target_str).map_err(|e| e.to_string())?;
            if out.json_mode {
                out.print_json(&outcome);
            } else {
                out.ok(&format!(
                    "wxwright MCP server written to {} (key {})",
                    outcome.config_path, outcome.key
                ));
                if outcome.backup_created {
                    out.info("previous config backed up as .json.bak");
                }
                if let Some(w) = &outcome.warning {
                    out.warn(w);
                }
                out.info("restart the client to pick up the new server");
            }
            Ok(0)
        }
    }
}

fn cmd_agent_card(md: bool, json: bool, out: &Out) -> Result<i32, String> {
    if json && !md {
        out.print_json(&wxwright_core::agentcard::card_json());
    } else {
        out.plain(&wxwright_core::agentcard::card_markdown());
    }
    Ok(0)
}

fn cmd_login(appid: &str, secret: &str, no_keyring: bool, out: &Out) -> Result<i32, String> {
    if appid.is_empty() || secret.is_empty() {
        return Err("--appid and --secret are both required".into());
    }
    wxwright_mp::save_credentials(
        &Credentials {
            appid: appid.to_string(),
            secret: secret.to_string(),
        },
        !no_keyring,
    )
    .map_err(|e| e.to_string())?;
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": true,
            "appid": appid,
            "secret": wxwright_mp::mask(secret),
            "storage": if no_keyring { "config-file" } else { "os-keychain" },
        }));
    } else {
        out.ok(&format!(
            "credentials stored (secret {}).",
            if no_keyring {
                "in config file"
            } else {
                "in OS keychain"
            }
        ));
        out.info("mmbiz upload and draft APIs are now unlocked");
    }
    Ok(0)
}

fn cmd_logout(out: &Out) -> Result<i32, String> {
    wxwright_mp::clear_credentials().map_err(|e| e.to_string())?;
    if out.json_mode {
        out.print_json(&serde_json::json!({ "ok": true }));
    } else {
        out.ok("credentials cleared");
    }
    Ok(0)
}

fn cmd_bench(iters: usize, out: &Out) -> Result<i32, String> {
    // ~10k-char sample document (PRD 5.7 P-2).
    let mut md = String::from("# 基准测试文章\n\n");
    let para = "这是一段用于性能基准测试的正文文字，包含**加粗**、*斜体*与`行内代码`，以及一个[链接](https://example.com/x)。\n\n";
    let list = "- 列表项一\n- 列表项二\n- [x] 已完成\n\n";
    for i in 0..60 {
        md.push_str(&format!("## 第 {} 节\n\n", i));
        md.push_str(para);
        md.push_str(list);
        md.push_str("> [!NOTE]\n> 提示卡片内容。\n\n");
        md.push_str("| 列A | 列B | 列C |\n|---|---|---|\n| 1 | 2 | 3 |\n\n");
        md.push_str("```text\nsome code line\nanother line\n```\n\n");
    }
    let opts = ConvertOptions::new(theme::load_theme("minimal").map_err(|e| e.to_string())?);
    // Warmup.
    let _ = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
    let mut samples: Vec<f64> = Vec::new();
    for _ in 0..iters {
        let start = std::time::Instant::now();
        let _ = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = samples[samples.len() / 2];
    let p95 = samples[(samples.len() as f64 * 0.95) as usize % samples.len()];
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": true,
            "doc_chars": md.chars().count(),
            "iterations": iters,
            "pipeline_ms_p50": p50,
            "pipeline_ms_p95": p95,
            "slo_p2_budget_ms": 60.0,
            "within_slo": p50 <= 60.0,
        }));
    } else {
        out.ok(&format!(
            "full chain over {} chars: p50 {:.1}ms, p95 {:.1}ms (SLO budget 60ms)",
            md.chars().count(),
            p50,
            p95
        ));
    }
    Ok(0)
}
