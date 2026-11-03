//! wxwright CLI: the agent-facing entry point (PRD 6).
//! Exit codes align with the official verify CLI: 0 pass / 1 violations / 2 error.

mod output;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use output::Out;
use wxwright_core::i18n::{set_lang, Lang};
use wxwright_core::img::ImageMode;
use wxwright_core::platform::{ExportKind, PlatformSpec};
use wxwright_core::theme;
use wxwright_core::validator::Violation;
use wxwright_core::{clipboard, ConvertOptions, PipelineOutput};
use wxwright_mp::{Credentials, MpClient};

#[derive(ValueEnum, Clone, Copy)]
enum LangArg {
    En,
    ZhCn,
}

#[derive(ValueEnum, Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    Markdown,
    Html,
}

#[derive(Parser)]
#[command(
    name = "wxwright",
    version,
    about = "Markdown to WeChat MP compliant rich text - CLI / MCP engine for agents and humans",
    long_about = None
)]
struct Cli {
    /// Human-output language for rule descriptions (en | zh-CN). Default: auto-detect.
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
    /// Markdown -> the target platform's artifact (WeChat MP dialect HTML by
    /// default; caption text or Markdown for the other platforms).
    Convert {
        /// Input markdown file, or "-" for stdin.
        input: String,
        /// Theme id (minimal|techblue|magazine) or path to a theme TOML.
        #[arg(long)]
        theme: Option<String>,
        /// Target platform id (see `wxwright platforms`). Default: wechat.
        #[arg(long)]
        platform: Option<String>,
        /// Source format. Omitted: frontmatter `format: html`, an .html/.htm
        /// extension, or (stdin) a leading "<" switches to html; otherwise
        /// markdown. HTML input skips the template renderer and runs the
        /// compliance normalizer + validator directly - arbitrary HTML from
        /// any tool goes in, official-spec HTML comes out. HTML is a
        /// WeChat-only entry; other platforms are generated from Markdown.
        #[arg(long, value_enum)]
        input_format: Option<InputFormat>,
        /// Output file, or "-" for stdout.
        #[arg(long, default_value = "-")]
        out: String,
    },
    /// Validate markdown/HTML against the official editor spec and the
    /// target platform's title/caption rules.
    Validate {
        /// Input .md or .html file, or "-" for stdin.
        input: String,
        /// Platform id for title/caption rules (default: frontmatter
        /// `platform`, else wechat). See `wxwright platforms`.
        #[arg(long)]
        platform: Option<String>,
        /// Exit 1 on warnings too. Blocking violations always exit 1.
        #[arg(long)]
        strict: bool,
        /// Note the official puppeteer bridge (CI truth) availability.
        #[arg(long)]
        official_check: bool,
    },
    /// Auto-fix violations in an HTML or Markdown file (normalizer report mode).
    Fix {
        input: String,
        /// Output file, or "-" for stdout.
        #[arg(long, default_value = "-")]
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
    /// Publish a draft via freepublish (发布). This makes the article public;
    /// it is not 群发 (mass send). Explicit confirmation required.
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
    /// Environment health check. Exit 0 healthy / 1 problems (CI/SCP gate).
    Doctor {
        /// Treat warnings as failures too (strict CI gate mode).
        #[arg(long)]
        strict: bool,
        /// Verify the stored MP credentials with a live token request.
        #[arg(long)]
        verify: bool,
    },
    /// List supported social platforms and their capabilities (PRD §16).
    Platforms,
    /// MCP server operations.
    Mcp {
        #[command(subcommand)]
        cmd: McpCmd,
    },
    /// Print the agent hand-off card. Plain Markdown by default in a
    /// terminal; JSON when stdout is piped or --json is passed.
    AgentCard {
        /// Force Markdown even when stdout is a pipe / --json is set.
        #[arg(long)]
        md: bool,
    },
    /// Store MP credentials in the OS keychain (AppID/Secret, PRD 5.6).
    Login {
        #[arg(long)]
        appid: String,
        /// Secret on the command line (visible in shell history and the
        /// process list; prefer --secret-stdin).
        #[arg(long)]
        secret: Option<String>,
        /// Read the secret from stdin instead of argv.
        #[arg(long)]
        secret_stdin: bool,
        /// Skip the live credential check (offline / CI).
        #[arg(long)]
        no_verify: bool,
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
        /// Image files to upload (at least one).
        #[arg(required = true)]
        paths: Vec<String>,
    },
    /// Fit the article's local image set to the platform's shape: centre-crop
    /// to the primary cover form and clamp the width (never upscale). Writes
    /// numbered files into --out-dir and prints a manifest.
    Fit {
        /// Input markdown file whose images get fitted.
        input: String,
        /// Target platform id (see `wxwright platforms`).
        #[arg(long)]
        platform: String,
        /// Directory to write the fitted images into.
        #[arg(long)]
        out_dir: String,
    },
}

#[derive(Subcommand)]
enum DraftCmd {
    /// Create a draft from markdown (images uploaded to mmbiz automatically).
    Create {
        /// Article file, or "-" for stdin. Frontmatter supplies title,
        /// author, digest and theme when the flags are omitted.
        #[arg(long)]
        file: String,
        /// Draft title (default: frontmatter `title`).
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        digest: Option<String>,
        #[arg(long)]
        theme: Option<String>,
        /// Original-article URL stored on the draft.
        #[arg(long)]
        source_url: Option<String>,
        /// thumb_media_id; defaults to the first uploaded article image.
        #[arg(long)]
        thumb_media_id: Option<String>,
        /// Cover image file to upload when the article has no image (required
        /// for HTML sources, which carry no uploadable image references).
        #[arg(long)]
        cover: Option<String>,
    },
    /// Update an existing draft. Metadata not passed on the command line is
    /// preserved from the draft being updated.
    Update {
        #[arg(long)]
        media_id: String,
        #[arg(long)]
        file: String,
        /// New title (default: the existing draft title).
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        digest: Option<String>,
        #[arg(long)]
        theme: Option<String>,
        #[arg(long)]
        source_url: Option<String>,
        /// New thumb_media_id (default: the existing draft cover).
        #[arg(long)]
        thumb_media_id: Option<String>,
        /// New cover image file to upload (default: the existing draft cover).
        #[arg(long)]
        cover: Option<String>,
        /// Article index inside a multi-article draft.
        #[arg(long, default_value = "0")]
        index: u64,
    },
    /// List drafts.
    List {
        #[arg(long, default_value = "0")]
        offset: u64,
        #[arg(long, default_value = "10")]
        count: u64,
    },
    /// Delete a draft by media_id (see `wxwright draft list`).
    Delete {
        #[arg(long)]
        media_id: String,
        /// Confirm the irreversible delete.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum ThemeCmd {
    List,
    /// Scaffold a new theme TOML.
    New {
        name: String,
        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
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
    let out = Out::new(cli.json, cli.no_color, cli.verbose);
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
            platform,
            input_format,
            out: out_path,
        } => cmd_convert(
            input,
            theme.as_deref(),
            platform.as_deref(),
            *input_format,
            out_path,
            out,
        ),
        Commands::Validate {
            input,
            platform,
            strict,
            official_check,
        } => cmd_validate(input, platform.as_deref(), *strict, *official_check, out),
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
            ImageCmd::Fit {
                input,
                platform,
                out_dir,
            } => cmd_image_fit(input, platform, out_dir, out),
        },
        Commands::Draft { cmd } => cmd_draft(cmd, out),
        Commands::Publish { draft_id, yes } => cmd_publish(draft_id, *yes, out),
        Commands::Theme { cmd } => cmd_theme(cmd, out),
        Commands::Doctor { strict, verify } => cmd_doctor(out, *strict, *verify),
        Commands::Platforms => cmd_platforms(out),
        Commands::Mcp { cmd } => cmd_mcp(cmd, out),
        Commands::AgentCard { md } => cmd_agent_card(*md, out),
        Commands::Login {
            appid,
            secret,
            secret_stdin,
            no_verify,
            no_keyring,
        } => cmd_login(
            appid,
            secret.as_deref(),
            *secret_stdin,
            *no_verify,
            *no_keyring,
            out,
        ),
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

/// Article-library files carry YAML frontmatter; split it for rendering.
fn split_frontmatter(md: &str) -> (Vec<(String, String)>, String) {
    wxwright_core::util::strip_frontmatter(md)
}

fn front_value<'a>(front: &'a [(String, String)], keys: &[&str]) -> Option<&'a str> {
    front
        .iter()
        .find(|(k, _)| keys.iter().any(|want| k.eq_ignore_ascii_case(want)))
        .map(|(_, v)| v.trim())
        .filter(|v| !v.is_empty())
}

fn theme_from(front: &[(String, String)], flag: Option<&str>) -> Option<String> {
    flag.map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| front_value(front, &["theme"]))
        .map(str::to_string)
}

fn detect_input_format(
    input: &str,
    front: &[(String, String)],
    flag: Option<InputFormat>,
    body: &str,
) -> InputFormat {
    if let Some(f) = flag {
        return f;
    }
    if let Some(fmt) = front_value(front, &["format"]) {
        if fmt.eq_ignore_ascii_case("html") {
            return InputFormat::Html;
        }
        if fmt.eq_ignore_ascii_case("markdown") || fmt.eq_ignore_ascii_case("md") {
            return InputFormat::Markdown;
        }
    }
    let lower = input.to_ascii_lowercase();
    if lower.ends_with(".html") || lower.ends_with(".htm") {
        return InputFormat::Html;
    }
    if lower.ends_with(".md") || lower.ends_with(".markdown") || lower.ends_with(".mdown") {
        return InputFormat::Markdown;
    }
    // stdin or an unknown extension: a leading tag means the source is already
    // HTML (the direct-entry mode), otherwise treat it as Markdown.
    if body.trim_start().starts_with('<') {
        return InputFormat::Html;
    }
    InputFormat::Markdown
}

fn platform_id_list() -> String {
    wxwright_core::platform::list_platforms()
        .iter()
        .map(|p| p.id)
        .collect::<Vec<_>>()
        .join(", ")
}

struct PlatformChoice {
    spec: PlatformSpec,
    known: bool,
    source: &'static str,
}

/// CLI flag > frontmatter > wechat. An explicitly passed unknown id is a
/// usage error (a typo must not silently render WeChat dialect); an unknown
/// id in frontmatter falls back with a warning, matching the GUI.
fn select_platform(
    flag: Option<&str>,
    front: &[(String, String)],
    out: &Out,
) -> Result<PlatformChoice, String> {
    if let Some(id) = flag.map(str::trim).filter(|s| !s.is_empty()) {
        let (spec, known) = wxwright_core::platform::resolve_platform(id);
        if !known {
            return Err(format!(
                "unknown platform {:?}; supported: {}",
                id,
                platform_id_list()
            ));
        }
        return Ok(PlatformChoice {
            spec,
            known,
            source: "flag",
        });
    }
    if let Some(id) = front_value(front, &["platform"]) {
        let (spec, known) = wxwright_core::platform::resolve_platform(id);
        if !known {
            out.warn(&format!(
                "unknown platform {:?} in frontmatter; falling back to {} - run `wxwright platforms` for the id list",
                id, spec.id
            ));
        }
        return Ok(PlatformChoice {
            spec,
            known,
            source: "frontmatter",
        });
    }
    Ok(PlatformChoice {
        spec: wxwright_core::platform::get_platform("wechat"),
        known: true,
        source: "default",
    })
}

fn title_violations(spec: &PlatformSpec, title: Option<&str>) -> Vec<Violation> {
    match title.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => wxwright_core::platform::validate_platform_title(spec.id, t),
        None => Vec::new(),
    }
}

fn blocking_count(v: &[Violation]) -> usize {
    v.iter().filter(|x| x.is_block()).count()
}

fn paste_hostile_sources(result: &PipelineOutput) -> Vec<String> {
    result
        .images
        .iter()
        .filter(|i| i.paste_hostile())
        .map(|i| i.source.clone())
        .collect()
}

/// A dry run must not touch WeChat's material library, so local images are
/// kept/inlined instead of uploaded. The engine's own prediction says which of
/// them a real run would have uploaded; everything else keeps the real verdict.
fn dry_run_hostile_sources(result: &PipelineOutput, uploads_enabled: bool) -> Vec<String> {
    result
        .images
        .iter()
        .filter(|i| i.dry_run_hostile(uploads_enabled))
        .map(|i| i.source.clone())
        .collect()
}

fn credential_rejected(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    ["40013", "40001", "40125", "invalid appid", "invalid secret"]
        .iter()
        .any(|needle| lower.contains(needle))
}

/// Write a file destination *before* anything is printed. Emitting the report
/// first made a failing `--out` print `ok: true` and then a second JSON
/// document, so an agent reading the first one believed the file existed.
fn write_artifact(out_path: &str, artifact: &str) -> Result<(), String> {
    if out_path == "-" {
        return Ok(());
    }
    std::fs::write(out_path, artifact).map_err(|e| format!("cannot write {}: {}", out_path, e))
}

/// Say where the artifact went, after the report. Stdout stays the single
/// JSON document; a file destination and the human-mode stream differ here.
fn announce_artifact(out_path: &str, artifact: &str, out: &Out, notes: &Out) -> Result<(), String> {
    if out_path == "-" {
        if !out.json_mode {
            std::io::stdout()
                .write_all(artifact.as_bytes())
                .map_err(|e| e.to_string())?;
            if !artifact.ends_with('\n') {
                println!();
            }
        }
    } else if !out.json_mode {
        notes.ok(&format!("written to {}", out_path));
    }
    Ok(())
}

fn violation_report<'a, I>(out: &Out, violations: I)
where
    I: IntoIterator<Item = &'a Violation>,
{
    for v in violations {
        // Platform rules (XHS-*) live outside the core RULES table; falling
        // back to the violation's own message keeps those lines readable
        // instead of printing an empty description.
        let desc = wxwright_core::rules::rule_info(&v.rule_id)
            .map(|r| r.localized_desc())
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| v.message.clone());
        let sev = if v.is_block() { "BLOCK" } else { "WARN" };
        out.note_plain(&format!(
            "  {} {} {} - {}",
            if v.is_block() { "[ERR]" } else { "[WARN]" },
            v.rule_id,
            sev,
            desc
        ));
        out.note_plain(&format!("      node: {}", v.node));
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
    platform: Option<&str>,
    input_format: Option<InputFormat>,
    out_path: &str,
    out: &Out,
) -> Result<i32, String> {
    let (raw, base_dir) = read_stdin_if_dash(input)?;
    let (front, md) = split_frontmatter(&raw);
    let fmt = detect_input_format(input, &front, input_format, &md);
    let notes = out.stderr_notes();
    let choice = select_platform(platform, &front, &notes)?;
    let spec = &choice.spec;
    let title = front_value(&front, &["title"]);
    let out_json = if out_path == "-" {
        serde_json::Value::Null
    } else {
        serde_json::json!(out_path)
    };
    out.diag(&format!(
        "convert: input={} format={} platform={} (source={}, known={}) title={:?}",
        input,
        fmt.as_str(),
        spec.id,
        choice.source,
        choice.known,
        title.unwrap_or("")
    ));

    // Direct-HTML entry: the source is already HTML, so the template renderer
    // is skipped - the compliance normalizer + validator are the whole chain
    // (PRD 5.1: the normalizer as the fixer for hand-authored HTML).
    if fmt == InputFormat::Html {
        if spec.id != "wechat" {
            return Err(format!(
                "direct HTML input is WeChat-only; platform {:?} is generated from Markdown. \
                 Use a .md source and drop --input-format html.",
                spec.id
            ));
        }
        if theme_name.is_some() {
            notes.warn(
                "--theme does not apply to direct HTML input (the template renderer is skipped)",
            );
        }
        let mut result = wxwright_core::pipeline_html(&md).map_err(|e| e.to_string())?;
        result.violations.extend(title_violations(spec, title));
        let blocking = blocking_count(&result.violations);
        let html = wxwright_core::wrap_document(&result.html, "#FFFFFF");
        write_artifact(out_path, &html)?;
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": blocking == 0,
                "platform": spec.id,
                "platform_known": choice.known,
                "platform_source": choice.source,
                "input_format": "html",
                "export_kind": "rich_text_dialect",
                "out": out_json,
                "html": html,
                "fixes": &result.fixes,
                "violations": &result.violations,
                "blocking_count": blocking,
            }));
        } else {
            if blocking == 0 {
                notes.ok(&format!(
                    "converted HTML ({} fix group(s))",
                    result.fixes.len()
                ));
            }
            print_violations_human(&notes, &result);
        }
        announce_artifact(out_path, &html, out, &notes)?;
        return Ok(if blocking > 0 { 1 } else { 0 });
    }

    // One engine, many exits (PRD 3.1): platforms that do not paste styled
    // HTML get the artifact they actually consume - a plain-text caption or
    // the Markdown unchanged.
    // One engine, three hosts: the artifact and the platform's caption rules
    // both come from core, so the CLI cannot drift from the MCP tool.
    if let Some((artifact, violations)) =
        wxwright_core::platform::export_text_artifact(spec.id, &md, title)
    {
        let kind = artifact.kind();
        let text = artifact.text().to_string();
        let mut all = violations;
        all.extend(wxwright_core::validator::validate_markdown(&md));
        // Caption exports already validate the title. Markdown hosts expose
        // no caption rules, but their title guidance still applies.
        if matches!(spec.export_kind, ExportKind::Markdown) {
            all.extend(title_violations(spec, title));
        }
        let blocking = blocking_count(&all);
        write_artifact(out_path, &text)?;
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": blocking == 0,
                "platform": spec.id,
                "platform_known": choice.known,
                "platform_source": choice.source,
                "export_kind": kind,
                "out": out_json,
                "title": title,
                "text": text,
                "violations": &all,
                "blocking_count": blocking,
            }));
        } else {
            notes.ok(&format!(
                "exported for {} ({} mode, {} chars)",
                spec.name_en,
                kind,
                text.chars().count()
            ));
            violation_report(&notes, &all);
        }
        announce_artifact(out_path, &text, out, &notes)?;
        // A blocking caption rule means the artifact cannot be posted as-is,
        // so the exit code has to say so - the same contract `validate` uses.
        return Ok(if blocking > 0 { 1 } else { 0 });
    }

    let theme_id = theme_from(&front, theme_name);
    let creds = wxwright_mp::load_credentials();
    let mode = if creds.is_some() {
        ImageMode::Upload
    } else {
        ImageMode::Inline
    };
    let opts = build_options(theme_id.as_deref(), mode, base_dir)?;
    out.diag(&format!(
        "images: mode={:?} (credentials {}) theme={}",
        mode,
        if creds.is_some() { "found" } else { "absent" },
        opts.theme.meta.id
    ));
    let mut result = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
    result
        .violations
        .extend(wxwright_core::validator::validate_markdown(&md));
    result.violations.extend(title_violations(spec, title));
    let blocking = result.blocking_violations().len();
    let hostile = paste_hostile_sources(&result);
    let html = wxwright_core::wrap_document(&result.html, opts.theme.canvas());
    write_artifact(out_path, &html)?;
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": blocking == 0,
            "platform": spec.id,
            "platform_known": choice.known,
            "platform_source": choice.source,
            "input_format": "markdown",
            "export_kind": "rich_text_dialect",
            "out": out_json,
            "theme": opts.theme.meta.id,
            "image_mode": match mode { ImageMode::Upload => "upload", _ => "inline" },
            "html": html,
            "stats": &result.stats,
            "images": &result.images,
            "paste_hostile_images": &hostile,
            "fixes": &result.fixes,
            "violations": &result.violations,
            "blocking_count": blocking,
        }));
    } else {
        notes.ok(&format!(
            "converted ({} chars, {} images, {} blocking violation(s))",
            result.stats.chars,
            result.images.len(),
            blocking
        ));
        if !hostile.is_empty() && mode != ImageMode::Upload {
            notes.warn(&format!(
                "{} image(s) are inlined as base64 and will not survive a paste into the MP editor; \
                 run `wxwright login` for mmbiz upload, or use `wxwright copy`",
                hostile.len()
            ));
        }
        print_violations_human(&notes, &result);
    }
    announce_artifact(out_path, &html, out, &notes)?;
    Ok(if blocking > 0 { 1 } else { 0 })
}

impl InputFormat {
    fn as_str(self) -> &'static str {
        match self {
            InputFormat::Markdown => "markdown",
            InputFormat::Html => "html",
        }
    }
}

fn cmd_validate(
    input: &str,
    platform: Option<&str>,
    strict: bool,
    official_check: bool,
    out: &Out,
) -> Result<i32, String> {
    let (raw, base_dir) = read_stdin_if_dash(input)?;
    let (front, md) = split_frontmatter(&raw);
    let fmt = detect_input_format(input, &front, None, &md);
    let choice = select_platform(platform, &front, out)?;
    let spec = &choice.spec;
    let title = front_value(&front, &["title"]);
    out.diag(&format!(
        "validate: input={} format={} platform={} (source={}) title={:?}",
        input,
        fmt.as_str(),
        spec.id,
        choice.source,
        title.unwrap_or("")
    ));

    let mut violations: Vec<Violation> = Vec::new();
    let mut caption: Option<String> = None;
    let export_kind = spec.export_kind;
    if fmt == InputFormat::Html {
        // validate_html is the WeChat official-spec checker: it is the only
        // HTML contract that exists, so a non-WeChat target can only be told
        // that caption rules need a Markdown source.
        if spec.export_kind != ExportKind::RichTextDialect {
            out.info(&format!(
                "HTML input is checked against the WeChat official spec; {} caption rules need a Markdown source",
                spec.id
            ));
        }
        violations.extend(wxwright_core::validator::validate_html(&md));
        violations.extend(title_violations(spec, title));
    } else {
        // Structure checks are independent of platform rendering: unclosed
        // fences and empty link targets survive every exporter.
        violations.extend(wxwright_core::validator::validate_markdown(&md));
        if let Some((artifact, caption_violations)) =
            wxwright_core::platform::export_text_artifact(spec.id, &md, title)
        {
            violations.extend(caption_violations);
            match spec.export_kind {
                ExportKind::Caption => caption = Some(artifact.text().to_string()),
                ExportKind::Markdown => violations.extend(title_violations(spec, title)),
                ExportKind::RichTextDialect => {}
            }
        } else {
            // Rich-text targets are checked through the exact convert chain,
            // so "validate passes" means "convert produces compliant HTML".
            let theme_id = theme_from(&front, None);
            let opts = build_options(theme_id.as_deref(), ImageMode::Inline, base_dir)?;
            let result = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
            violations.extend(result.violations);
            violations.extend(title_violations(spec, title));
        }
    };

    let blocking = blocking_count(&violations);
    let warnings = violations.len().saturating_sub(blocking);
    // Blocking always fails; warnings fail only under --strict. The old
    // implementation only failed when --strict was on *and* a block existed,
    // so a strict CI gate silently passed broken articles.
    let failed = blocking > 0 || (strict && warnings > 0);
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": !failed,
            "compliant": blocking == 0,
            "platform": spec.id,
            "platform_known": choice.known,
            "platform_source": choice.source,
            "input_format": fmt.as_str(),
            "export_kind": export_kind,
            "title": title,
            "strict": strict,
            "violations": &violations,
            "blocking_count": blocking,
            "warnings_count": warnings,
            "caption": caption,
        }));
    } else if failed {
        if blocking > 0 {
            out.err(&format!(
                "{} blocking violation(s), {} warning(s)",
                blocking, warnings
            ));
        } else {
            out.err(&format!("{} warning(s) and --strict is on", warnings));
        }
        violation_report(out, &violations);
    } else {
        out.ok(&format!("compliant ({} warning(s))", warnings));
        violation_report(out, violations.iter().filter(|v| !v.is_block()));
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

    Ok(if failed { 1 } else { 0 })
}

fn cmd_fix(input: &str, out_path: &str, out: &Out) -> Result<i32, String> {
    let (raw, base_dir) = read_stdin_if_dash(input)?;
    let (front, body) = split_frontmatter(&raw);
    let fmt = detect_input_format(input, &front, None, &body);
    let notes = out.stderr_notes();
    let (html, mut violations, fixes) = if fmt == InputFormat::Html {
        let result = wxwright_core::normalizer::normalize_html(&body, Default::default())
            .map_err(|e| e.to_string())?;
        let violations = wxwright_core::validator::validate_html(&result.html);
        (result.html, violations, result.fixes)
    } else {
        // Markdown repair runs the full render -> normalize -> validate chain,
        // so `fix article.md` returns an already-fixed HTML artifact instead
        // of demanding a separate convert step.
        let theme_id = theme_from(&front, None);
        let opts = build_options(theme_id.as_deref(), ImageMode::Inline, base_dir)?;
        let result = wxwright_core::pipeline(&body, &opts).map_err(|e| e.to_string())?;
        (result.html, result.violations, result.fixes)
    };
    if fmt == InputFormat::Markdown {
        violations.extend(wxwright_core::validator::validate_markdown(&body));
        violations.extend(title_violations(
            &wxwright_core::platform::get_platform("wechat"),
            front_value(&front, &["title"]),
        ));
    }
    let blocking = blocking_count(&violations);
    let out_json = if out_path == "-" {
        serde_json::Value::Null
    } else {
        serde_json::json!(out_path)
    };
    write_artifact(out_path, &html)?;
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": blocking == 0,
            "input_format": fmt.as_str(),
            "out": out_json,
            "html": &html,
            "fixes": &fixes,
            "remaining_violations": &violations,
            "blocking_count": blocking,
        }));
    } else {
        notes.ok(&format!("fixed {} issue group(s)", fixes.len()));
        for f in &fixes {
            notes.info(&format!("[{}] x{}: {}", f.rule_id, f.count, f.detail));
        }
        if blocking > 0 {
            notes.err(&format!(
                "{} blocking issue(s) need manual attention",
                blocking
            ));
            violation_report(&notes, violations.iter().filter(|v| v.is_block()));
        } else if !violations.is_empty() {
            notes.warn(&format!("{} warning(s) remain", violations.len()));
            violation_report(&notes, &violations);
        }
    }
    announce_artifact(out_path, &html, out, &notes)?;
    Ok(if blocking > 0 { 1 } else { 0 })
}

fn cmd_copy(
    input: &str,
    theme_name: Option<&str>,
    dry_run: bool,
    out: &Out,
) -> Result<i32, String> {
    let (raw, base_dir) = read_stdin_if_dash(input)?;
    let (front, md) = split_frontmatter(&raw);
    let theme_id = theme_from(&front, theme_name);
    let title = front_value(&front, &["title"]);
    let creds = wxwright_mp::load_credentials();
    let has_creds = creds.is_some();
    // A dry run must not upload anything: use Keep (local files are inlined,
    // no network IO) and predict the upload outcome at the image gate below.
    let mode = if dry_run {
        ImageMode::Keep
    } else if has_creds {
        ImageMode::Upload
    } else {
        ImageMode::Keep
    };
    let opts = build_options(theme_id.as_deref(), mode, base_dir)?;
    let theme_id_used = opts.theme.meta.id.clone();
    let mut result = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
    result
        .violations
        .extend(wxwright_core::validator::validate_markdown(&md));
    result.violations.extend(title_violations(
        &wxwright_core::platform::get_platform("wechat"),
        title,
    ));

    // Blocking violations: never copy a non-compliant payload.
    let blocks = result.blocking_violations();
    if !blocks.is_empty() {
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": false, "copied": false, "dry_run": dry_run,
                "reason": "blocking violations",
                "violations": &result.violations,
                "fixes": &result.fixes,
                "blocking_count": blocks.len(),
            }));
        } else {
            out.err("copy blocked: blocking violations (fix the markdown first)");
            print_violations_human(out, &result);
        }
        return Ok(1);
    }

    // I-03: clipboard payload must be mmbiz or https images; base64 payloads,
    // plain-http hosts and images that failed to load all break on paste.
    // The predicate lives in the engine so the GUI paths cannot drift away
    // from it. A dry run predicts the real outcome instead of skipping the
    // gate and reporting a false success.
    let paste_hostile = if dry_run {
        dry_run_hostile_sources(&result, has_creds)
    } else {
        paste_hostile_sources(&result)
    };
    if !paste_hostile.is_empty() {
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": false, "copied": false, "dry_run": dry_run,
                "reason": "paste-hostile images: clipboard requires mmbiz or https URLs (I-03)",
                "local_images": &paste_hostile,
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
            out.print_json(&serde_json::json!({
                "ok": true, "dry_run": true, "copied": false,
                "platform": "wechat",
                "title": title,
                "theme": theme_id_used,
                "uploads_skipped": has_creds,
                "note": if has_creds {
                    "dry run did not upload; local images are assumed to upload successfully - a failed upload still blocks the real copy"
                } else {
                    "no credentials: local images would be inlined and blocked by the real copy"
                },
                "report": report_payload(&result),
            }));
        } else {
            out.ok("dry run passed: chain is clean, clipboard not touched");
            if has_creds {
                out.info("image uploads were not performed (dry run)");
            }
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
    let creds = wxwright_mp::load_credentials().ok_or(wxwright_mp::NO_CREDENTIALS_HINT)?;
    let client = MpClient::new(creds);
    let mut results = Vec::new();
    for p in paths {
        let name = PathBuf::from(p)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "image.png".into());
        // One unreadable path must not discard the uploads that already
        // succeeded: report it as that entry's error and keep going.
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => {
                results.push(
                    serde_json::json!({ "source": p, "ok": false, "error": format!("cannot read {}: {}", p, e) }),
                );
                continue;
            }
        };
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

/// `wxwright image fit`: the cross-platform crop/fit step. Every image the
/// article references as a local file is centre-cropped to the platform's
/// primary cover shape (rich-text platforms: width clamp only) and written
/// into --out-dir in document order. Remote/inline images are reported as
/// skipped - fitting those would mean fetching, which core never does.
fn cmd_image_fit(input: &str, platform: &str, out_dir: &str, out: &Out) -> Result<i32, String> {
    let (spec, known) = wxwright_core::platform::resolve_platform(platform);
    if !known {
        return Err(format!(
            "unknown platform {:?}; supported: {}",
            platform,
            platform_id_list()
        ));
    }
    let profile = spec.image_profile();
    let (raw, base_dir) = read_stdin_if_dash(input)?;
    let (front, body) = split_frontmatter(&raw);
    if detect_input_format(input, &front, None, &body) == InputFormat::Html {
        return Err(
            "image fit reads Markdown image references; HTML input is not supported".into(),
        );
    }
    let doc = wxwright_core::parser::parse_markdown(&body);
    let images = wxwright_core::ir::collect_images(&doc);
    if images.is_empty() {
        return Err(format!("{input} contains no images to fit"));
    }
    std::fs::create_dir_all(out_dir).map_err(|e| format!("cannot create {out_dir}: {e}"))?;

    // Image sources are resolved like the renderer resolves them: relative
    // to the markdown file's directory (cwd for stdin).
    let base = base_dir.unwrap_or_else(|| PathBuf::from("."));
    let mut manifest = Vec::new();
    let mut fitted = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut seq = 0usize;
    for img in &images {
        let src = img.src.as_str();
        if src.starts_with("http://")
            || src.starts_with("https://")
            || src.starts_with("data:")
            || src.is_empty()
        {
            skipped += 1;
            // Skipping a remote/inline image is the documented behaviour, not
            // a failure; `failed` is the only per-entry outcome that fails.
            manifest.push(serde_json::json!({
                "source": src, "ok": true,
                "skipped": "remote or inline image; only local files are fitted",
            }));
            continue;
        }
        let path = base.join(src);
        let entry = match std::fs::read(&path) {
            Ok(bytes) => match wxwright_core::img::fit_to_profile(&bytes, &profile) {
                Ok(fit) => {
                    seq += 1;
                    let stem = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "image".into());
                    let name = format!("{:02}-{}.{}", seq, stem, fitted_ext(&fit.bytes, &path));
                    let dest = Path::new(out_dir).join(&name);
                    match std::fs::write(&dest, &fit.bytes) {
                        Ok(()) => {
                            fitted += 1;
                            serde_json::json!({
                                "source": src, "ok": true, "file": dest.display().to_string(),
                                "alt": img.alt, "width": fit.width, "height": fit.height,
                                "cropped": fit.cropped, "resized": fit.resized,
                            })
                        }
                        Err(e) => {
                            failed += 1;
                            serde_json::json!({
                                "source": src, "ok": false, "error": format!("cannot write {}: {}", dest.display(), e),
                            })
                        }
                    }
                }
                Err(e) => {
                    failed += 1;
                    serde_json::json!({ "source": src, "ok": false, "error": e.to_string() })
                }
            },
            Err(e) => {
                failed += 1;
                serde_json::json!({
                    "source": src, "ok": false,
                    "error": format!("cannot read {}: {}", path.display(), e),
                })
            }
        };
        manifest.push(entry);
    }
    // Skips are informational; only a local image that failed to fit or
    // write makes the command fail. The old rule ("at least one fitted")
    // reported failure for an article whose images are all remote.
    let ok = failed == 0;
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": ok,
            "platform": spec.id,
            "out_dir": out_dir,
            "profile": { "max_width": profile.max_width, "cover_aspect": profile.cover_aspect },
            "fitted": fitted,
            "skipped": skipped,
            "failed": failed,
            "images": manifest,
        }));
    } else {
        if ok {
            out.ok(&format!(
                "{}: fitted {} image(s), skipped {} ({})",
                spec.id,
                fitted,
                skipped,
                match profile.cover_aspect {
                    Some((w, h)) => format!("cover {}:{}", w, h),
                    None => "width clamp only".to_string(),
                }
            ));
        } else {
            out.err(&format!(
                "{}: fitted {}, skipped {}, failed {}",
                spec.id, fitted, skipped, failed
            ));
        }
        for e in &manifest {
            if e["ok"] == serde_json::json!(true) {
                out.ok(&format!(
                    "{} -> {} ({}x{}{}{})",
                    e["source"].as_str().unwrap_or(""),
                    e["file"].as_str().unwrap_or(""),
                    e["width"],
                    e["height"],
                    if e["cropped"] == serde_json::json!(true) {
                        ", cropped"
                    } else {
                        ""
                    },
                    if e["resized"] == serde_json::json!(true) {
                        ", resized"
                    } else {
                        ""
                    }
                ));
            } else if let Some(why) = e["skipped"].as_str() {
                out.info(&format!(
                    "{} -> skipped: {}",
                    e["source"].as_str().unwrap_or(""),
                    why
                ));
            } else {
                out.err(&format!(
                    "{} -> {}",
                    e["source"].as_str().unwrap_or(""),
                    e["error"].as_str().unwrap_or("failed")
                ));
            }
        }
    }
    Ok(if ok { 0 } else { 1 })
}

/// Extension of the bytes actually written: fit_to_profile passes the
/// original file through untouched when no crop/resize applies, so the input
/// format is not a reliable guess.
fn fitted_ext(bytes: &[u8], src: &Path) -> String {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return "png".into();
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return "jpg".into();
    }
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "webp" | "bmp" | "tif" | "tiff" => ext,
        _ => "img".into(),
    }
}

/// The draft gates every MP write path shares (pitfall 37): blocking
/// violations, then paste-hostile images. Direct-HTML sources bypass the
/// image pipeline, so their `src` attributes need the separate (quote-aware)
/// HTML scanner.
fn draft_gates_pass(out: &Out, result: &PipelineOutput, html_mode: bool) -> bool {
    if !result.blocking_violations().is_empty() {
        out.err("draft blocked: blocking violations");
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": false,
                "reason": "blocking violations",
                "report": report_payload(result),
            }));
        } else {
            print_violations_human(out, result);
        }
        return false;
    }
    let hostile: Vec<String> = if html_mode {
        wxwright_core::htmlutil::paste_hostile_html_imgs(&result.html)
    } else {
        result
            .images
            .iter()
            .filter(|i| i.paste_hostile())
            .map(|i| i.source.clone())
            .collect()
    };
    if !hostile.is_empty() {
        out.err(&format!(
            "draft blocked: {} image(s) cannot display in the MP editor (need mmbiz or an https URL): {}",
            hostile.len(),
            hostile.join("; ")
        ));
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": false,
                "reason": "paste-hostile images",
                "images": &hostile,
            }));
        }
        return false;
    }
    true
}

struct RenderedDraft {
    result: PipelineOutput,
    front: Vec<(String, String)>,
    title: Option<String>,
    format: InputFormat,
}

/// Render an article for the draft box: Markdown goes through the same
/// upload -> normalize -> validate chain as `convert`, direct HTML through
/// the normalizer only (PRD 5.1). Metadata stays in `front` for the caller.
fn render_draft(
    file: &str,
    raw: &str,
    base_dir: Option<PathBuf>,
    theme_id: Option<&str>,
    creds: &Credentials,
) -> Result<RenderedDraft, String> {
    let (front, body) = split_frontmatter(raw);
    let fmt = detect_input_format(file, &front, None, &body);
    let title = front_value(&front, &["title"]).map(str::to_string);
    let mut result = if fmt == InputFormat::Html {
        let norm = wxwright_core::normalizer::normalize_html(&body, Default::default())
            .map_err(|e| e.to_string())?;
        let violations = wxwright_core::validator::validate_html(&norm.html);
        PipelineOutput {
            html: norm.html,
            images: Vec::new(),
            stats: wxwright_core::ir::Stats::default(),
            links: Vec::new(),
            fixes: norm.fixes,
            violations,
        }
    } else {
        let t = theme::load_theme(theme_id.unwrap_or("minimal")).map_err(|e| e.to_string())?;
        let mut opts = ConvertOptions::new(t);
        opts.image_mode = ImageMode::Upload;
        opts.base_dir = base_dir;
        opts.transport = Some(Arc::new(MpClient::new(creds.clone())));
        wxwright_core::pipeline(&body, &opts).map_err(|e| e.to_string())?
    };
    result
        .violations
        .extend(wxwright_core::validator::validate_markdown(&body));
    Ok(RenderedDraft {
        result,
        front,
        title,
        format: fmt,
    })
}

fn front_digest(front: &[(String, String)]) -> Option<String> {
    front_value(front, &["digest", "summary", "description"]).map(|s| s.chars().take(120).collect())
}

struct ExistingDraft {
    title: Option<String>,
    author: Option<String>,
    digest: Option<String>,
    source_url: Option<String>,
    thumb: Option<String>,
}

/// Fetch the draft being updated so flags the caller omitted keep the
/// existing values instead of blanking the byline, summary, source URL or
/// cover.
fn existing_draft(client: &MpClient, media_id: &str, index: u64) -> Result<ExistingDraft, String> {
    let v = client.draft_get(media_id).map_err(|e| e.to_string())?;
    let items = v
        .get("news_item")
        .and_then(|n| n.as_array())
        .or_else(|| {
            v.get("content")
                .and_then(|c| c.get("news_item"))
                .and_then(|n| n.as_array())
        })
        .ok_or_else(|| {
            format!(
                "draft {} has no news_item array (unexpected draft/get response)",
                media_id
            )
        })?;
    let item = items.get(index as usize).ok_or_else(|| {
        format!(
            "draft {} has {} article(s); index {} is out of range",
            media_id,
            items.len(),
            index
        )
    })?;
    let s = |k: &str| {
        item.get(k)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .filter(|v| !v.is_empty())
    };
    Ok(ExistingDraft {
        title: s("title"),
        author: s("author"),
        digest: s("digest"),
        source_url: s("content_source_url"),
        thumb: s("thumb_media_id"),
    })
}

fn upload_cover(client: &MpClient, path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read cover {}: {}", path, e))?;
    let name = Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "cover.png".into());
    client
        .upload_material(bytes, &name)
        .map(|(id, _)| id)
        .map_err(|e| format!("cover upload failed: {}", e))
}

fn cmd_draft(cmd: &DraftCmd, out: &Out) -> Result<i32, String> {
    // Confirmation is a usage decision: check it before credentials so a
    // missing config never masks "rerun with --yes".
    if matches!(cmd, DraftCmd::Delete { yes: false, .. }) {
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": false,
                "error": "draft delete is irreversible; rerun with --yes",
            }));
        } else {
            out.err("draft delete is irreversible - rerun with --yes");
        }
        return Ok(2);
    }
    let creds = wxwright_mp::load_credentials().ok_or(wxwright_mp::NO_CREDENTIALS_HINT)?;
    let client = MpClient::new(creds.clone());
    match cmd {
        DraftCmd::Create {
            file,
            title,
            author,
            digest,
            theme: theme_name,
            source_url,
            thumb_media_id,
            cover,
        } => {
            let (raw, base_dir) = read_stdin_if_dash(file)?;
            let theme_id = theme_from(&split_frontmatter(&raw).0, theme_name.as_deref());
            let mut rendered = render_draft(file, &raw, base_dir, theme_id.as_deref(), &creds)?;
            let title = title
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .or(rendered.title.clone())
                .ok_or("draft needs a title: pass --title or add `title:` to the frontmatter")?;
            rendered.result.violations.extend(title_violations(
                &wxwright_core::platform::get_platform("wechat"),
                Some(&title),
            ));
            if !draft_gates_pass(out, &rendered.result, rendered.format == InputFormat::Html) {
                return Ok(1);
            }
            let author = author
                .clone()
                .or_else(|| front_value(&rendered.front, &["author"]).map(str::to_string))
                .unwrap_or_default();
            let digest = digest
                .clone()
                .or_else(|| front_digest(&rendered.front))
                .map(|d| d.chars().take(120).collect())
                .unwrap_or_default();
            let source_url = source_url
                .clone()
                .or_else(|| {
                    front_value(&rendered.front, &["source_url", "source"]).map(str::to_string)
                })
                .unwrap_or_default();
            let thumb = match thumb_media_id
                .clone()
                .or_else(|| rendered.result.images.iter().find_map(|i| i.media_id.clone()))
            {
                Some(t) => t,
                None => match cover.as_deref() {
                    Some(path) => upload_cover(&client, path)?,
                    None => return Err(
                        "no cover available: add an image to the article, pass --cover <file>, or --thumb-media-id <id>"
                            .into(),
                    ),
                },
            };
            let article = wxwright_mp::DraftArticle {
                title,
                author,
                digest,
                content_html: rendered.result.html.clone(),
                content_source_url: source_url,
                thumb_media_id: thumb,
            };
            let media_id = client.draft_add(&article).map_err(|e| e.to_string())?;
            if out.json_mode {
                out.print_json(&serde_json::json!({
                    "ok": true,
                    "draft_media_id": media_id,
                    "report": report_payload(&rendered.result),
                }));
            } else {
                out.ok(&format!("draft created: {}", media_id));
                print_violations_human(out, &rendered.result);
                out.info("open MP -> 草稿箱 (Drafts) to review and publish");
            }
            Ok(0)
        }
        DraftCmd::Update {
            media_id,
            file,
            title,
            author,
            digest,
            theme: theme_name,
            source_url,
            thumb_media_id,
            cover,
            index,
        } => {
            let existing = existing_draft(&client, media_id, *index)?;
            let (raw, base_dir) = read_stdin_if_dash(file)?;
            let theme_id = theme_from(&split_frontmatter(&raw).0, theme_name.as_deref());
            let mut rendered = render_draft(file, &raw, base_dir, theme_id.as_deref(), &creds)?;
            let title = title
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .or(rendered.title.clone())
                .or(existing.title.clone())
                .ok_or("draft needs a title: pass --title or add `title:` to the frontmatter")?;
            rendered.result.violations.extend(title_violations(
                &wxwright_core::platform::get_platform("wechat"),
                Some(&title),
            ));
            if !draft_gates_pass(out, &rendered.result, rendered.format == InputFormat::Html) {
                return Ok(1);
            }
            let author = author
                .clone()
                .or_else(|| front_value(&rendered.front, &["author"]).map(str::to_string))
                .or(existing.author.clone())
                .unwrap_or_default();
            let digest = digest
                .clone()
                .or_else(|| front_digest(&rendered.front))
                .or(existing.digest.clone())
                .map(|d| d.chars().take(120).collect())
                .unwrap_or_default();
            let source_url = source_url
                .clone()
                .or_else(|| {
                    front_value(&rendered.front, &["source_url", "source"]).map(str::to_string)
                })
                .or(existing.source_url.clone())
                .unwrap_or_default();
            let thumb = match thumb_media_id
                .clone()
                .or_else(|| rendered.result.images.iter().find_map(|i| i.media_id.clone()))
            {
                Some(t) => t,
                None => match cover.as_deref() {
                    Some(path) => upload_cover(&client, path)?,
                    None => existing.thumb.clone().ok_or_else(|| {
                        format!(
                            "draft {} has no readable cover; pass --cover <file> or --thumb-media-id <id>",
                            media_id
                        )
                    })?,
                },
            };
            let article = wxwright_mp::DraftArticle {
                title,
                author,
                digest,
                content_html: rendered.result.html.clone(),
                content_source_url: source_url,
                thumb_media_id: thumb,
            };
            client
                .draft_update(media_id, *index, &article)
                .map_err(|e| e.to_string())?;
            if out.json_mode {
                out.print_json(&serde_json::json!({
                    "ok": true,
                    "updated": media_id,
                    "index": index,
                    "report": report_payload(&rendered.result),
                }));
            } else {
                out.ok(&format!(
                    "draft {} updated (article index {})",
                    media_id, index
                ));
                print_violations_human(out, &rendered.result);
            }
            Ok(0)
        }
        DraftCmd::List { offset, count } => {
            let mut v = client
                .draft_list(*offset, *count)
                .map_err(|e| e.to_string())?;
            if out.json_mode {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("ok".into(), serde_json::json!(true));
                }
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
        DraftCmd::Delete { media_id, .. } => {
            client.draft_delete(media_id).map_err(|e| e.to_string())?;
            if out.json_mode {
                out.print_json(&serde_json::json!({ "ok": true, "deleted": media_id }));
            } else {
                out.ok(&format!("draft deleted: {}", media_id));
            }
            Ok(0)
        }
    }
}

fn cmd_publish(draft_id: &str, yes: bool, out: &Out) -> Result<i32, String> {
    if !yes {
        if out.json_mode {
            out.print_json(&serde_json::json!({
                "ok": false,
                "error": "publish requires explicit --yes; it submits the draft to freepublish (发布, not 群发)"
            }));
        } else {
            out.err(
                "publish submits the draft for 发布 (freepublish) - rerun with --yes to confirm",
            );
        }
        return Ok(2);
    }
    let creds = wxwright_mp::load_credentials().ok_or(wxwright_mp::NO_CREDENTIALS_HINT)?;
    let client = MpClient::new(creds);
    let resp = client
        .freepublish_submit(draft_id)
        .map_err(|e| e.to_string())?;
    if out.json_mode {
        let mut payload = resp;
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("ok".into(), serde_json::json!(true));
            obj.insert("draft_id".into(), serde_json::json!(draft_id));
        }
        out.print_json(&payload);
    } else {
        out.ok(&format!("publish submitted for draft {}", draft_id));
        if let Some(publish_id) = resp.get("publish_id").and_then(|v| v.as_str()) {
            out.info(&format!("publish_id: {}", publish_id));
        }
        out.info("check the MP console -> 发表记录 (Publish Records) for status");
    }
    Ok(0)
}

fn cmd_theme(cmd: &ThemeCmd, out: &Out) -> Result<i32, String> {
    match cmd {
        ThemeCmd::List => {
            let themes = theme::builtin_themes().map_err(|e| e.to_string())?;
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
                out.print_json(&serde_json::json!({ "ok": true, "themes": list }));
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
            Ok(0)
        }
        ThemeCmd::New { name, force } => {
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(
                    "theme name may only contain letters, digits, '-' and '_' (it becomes <name>.toml)"
                        .into(),
                );
            }
            let path = format!("{}.toml", name);
            let body = theme_scaffold(name);
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true);
            if *force {
                opts.create(true).truncate(true);
            } else {
                opts.create_new(true);
            }
            let mut file = opts.open(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    format!("{} already exists; pass --force to overwrite", path)
                } else {
                    format!("cannot write {}: {}", path, e)
                }
            })?;
            file.write_all(body.as_bytes())
                .map_err(|e| format!("cannot write {}: {}", path, e))?;
            if out.json_mode {
                out.print_json(&serde_json::json!({
                    "ok": true,
                    "path": path,
                    "overwritten": *force,
                }));
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

# Content variants: the pull quote and the lede are where a theme shows its
# character - restyle these four keys with your own palette.
[block.quote.hero]
background = "none"
border-left = "4px solid {{accent}}"
padding = "4px 0"

[block.quote.hero_leaf]
font-size = "20px"
font-weight = "600"

[block.paragraph.lead]
font-size = "17px"
color = "{{text_secondary}}"

[block.paragraph.lead_leaf]
font-size = "17px"

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

fn cmd_platforms(out: &Out) -> Result<i32, String> {
    if out.json_mode {
        out.print_json(&wxwright_core::platform::list_platforms());
        return Ok(0);
    }
    out.plain(&format!(
        "{:<10} {:<42} {:<22} {}",
        "ID", "PLATFORM", "OUTPUT", "PRESETS"
    ));
    for p in wxwright_core::platform::list_platforms() {
        let output = if p.rich_text {
            "rich text"
        } else if p.image_note {
            "image note"
        } else {
            "images/text"
        };
        let api = if p.api_publish { "+api" } else { "-" };
        let example = p
            .presets
            .first()
            .map(|(l, w, h)| format!("{} ({}x{})", l, w, h))
            .unwrap_or_default();
        out.plain(&format!(
            "{:<10} {:<42} {:<22} {:>2}  {}",
            p.id,
            format!("{} / {}", p.name_zh, p.name_en),
            format!("{} {}", output, api),
            p.presets.len(),
            example
        ));
    }
    Ok(0)
}

fn cmd_doctor(out: &Out, strict: bool, verify: bool) -> Result<i32, String> {
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
        Some(c) if verify => {
            let appid = c.appid.clone();
            let secret = wxwright_mp::mask(&c.secret);
            let client = MpClient::new(c);
            match client.access_token(false) {
                Ok(_) => {
                    let nickname = client.account_basic_info().ok().map(|(name, _)| name);
                    push(
                        "credentials",
                        "ok",
                        serde_json::json!({
                            "appid": appid,
                            "secret": secret,
                            "verified": true,
                            "nickname": nickname,
                        }),
                    );
                }
                Err(e) => {
                    let msg = e.to_string();
                    if credential_rejected(&msg) {
                        push(
                            "credentials",
                            "err",
                            serde_json::json!({
                                "appid": appid,
                                "secret": secret,
                                "verified": false,
                                "error": msg,
                                "hint": "the AppID/Secret was rejected; rerun `wxwright login` with the current Secret",
                            }),
                        );
                    } else {
                        // The token endpoint failing for any other reason
                        // (network, 40164 IP whitelist) says nothing about
                        // whether the credentials themselves are valid.
                        let hint = if msg.contains("40164") {
                            "add this machine's public IP to the MP API whitelist, then rerun with --verify"
                        } else {
                            "network or API problem; the credentials were not judged"
                        };
                        push(
                            "credentials",
                            "warn",
                            serde_json::json!({
                                "appid": appid,
                                "secret": secret,
                                "verified": false,
                                "error": msg,
                                "hint": hint,
                            }),
                        );
                    }
                }
            }
        }
        Some(c) => push(
            "credentials",
            "ok",
            serde_json::json!({
                "appid": c.appid,
                "secret": wxwright_mp::mask(&c.secret),
                "verified": false,
                "note": "presence only; pass --verify for a live token check",
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

    // State directory must be writable: a read-only %APPDATA% silently breaks
    // theme saving and credential storage. This is the one check worth
    // failing on, so `doctor` can act as a CI / SCP preflight gate.
    let state_dir = wxwright_core::util::config_root();
    let state_ok = std::fs::create_dir_all(&state_dir).is_ok();
    push(
        "state_dir",
        if state_ok { "ok" } else { "err" },
        serde_json::json!({
            "path": state_dir.to_string_lossy(),
            "writable": state_ok,
            "purpose": "settings / credentials / user themes",
        }),
    );

    // AI provider is a desktop-app concern - the CLI itself never calls an
    // LLM - so a missing configuration is a warning, not a failure.
    let settings = state_dir.join("settings.json");
    let settings_exists = settings.exists();
    push(
        "ai_provider",
        if settings_exists { "ok" } else { "warn" },
        serde_json::json!({
            "settings_file": settings.to_string_lossy(),
            "configured": settings_exists,
            "purpose": "desktop app AI assistant, theme and image generation",
            "hint": "configure a provider in the app: Settings -> AI Providers",
        }),
    );

    let errors = checks.iter().any(|c| c["status"] == "err");
    let warnings = checks.iter().any(|c| c["status"] == "warn");
    let failed = errors || (strict && warnings);
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": !failed,
            "errors": errors,
            "warnings": warnings,
            "strict": strict,
            "checks": checks,
        }));
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
        if failed {
            out.err(if errors {
                "doctor: problems found (exit 1)"
            } else {
                "doctor: warnings present and --strict is on (exit 1)"
            });
        }
    }
    Ok(if failed { 1 } else { 0 })
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
                let mut v = serde_json::to_value(&outcome).map_err(|e| e.to_string())?;
                v["ok"] = serde_json::json!(true);
                out.print_json(&v);
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

fn cmd_agent_card(md: bool, out: &Out) -> Result<i32, String> {
    // Markdown is the human/terminal form; JSON is what an agent consumes
    // through a pipe. --md pins the Markdown even under --json.
    if md || !out.json_mode {
        out.plain(&wxwright_core::agentcard::card_markdown());
    } else {
        out.print_json(&wxwright_core::agentcard::card_json());
    }
    Ok(0)
}

fn cmd_login(
    appid: &str,
    secret: Option<&str>,
    secret_stdin: bool,
    no_verify: bool,
    no_keyring: bool,
    out: &Out,
) -> Result<i32, String> {
    let appid = appid.trim();
    if appid.is_empty() {
        return Err("--appid must not be empty".into());
    }
    if secret.is_some() && secret_stdin {
        return Err("pass either --secret or --secret-stdin, not both".into());
    }
    let secret = if secret_stdin {
        // Reading from stdin keeps the secret out of shell history and the
        // process list (PRD 5.6 / pitfall 16).
        let mut buf = String::new();
        std::io::stdin()
            .read_line(&mut buf)
            .map_err(|e| format!("cannot read the secret from stdin: {}", e))?;
        buf.trim().to_string()
    } else {
        secret
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                std::env::var("WXWRIGHT_MP_SECRET")
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
            .ok_or(
                "no secret given: use --secret-stdin (recommended), --secret, or WXWRIGHT_MP_SECRET",
            )?
    };
    if secret.is_empty() {
        return Err("the secret must not be empty".into());
    }

    let creds = Credentials {
        appid: appid.to_string(),
        secret: secret.clone(),
    };
    // Verify before storing: a typo that overwrites working credentials is a
    // silent outage. IP-whitelist and network failures still store (the
    // credentials themselves are valid; the caller can fix the whitelist).
    let (verified, nickname, warning): (bool, Option<String>, Option<String>) = if no_verify {
        (
            false,
            None,
            Some("verification skipped (--no-verify)".into()),
        )
    } else {
        let client = MpClient::new(creds.clone());
        match client.access_token(false) {
            Ok(_) => (
                true,
                client.account_basic_info().ok().map(|(name, _)| name),
                None,
            ),
            Err(e) => {
                let msg = e.to_string();
                if credential_rejected(&msg) {
                    return Err(format!(
                        "credential check failed; nothing was stored: {}",
                        msg
                    ));
                }
                (false, None, Some(msg))
            }
        }
    };

    wxwright_mp::save_credentials(&creds, !no_keyring).map_err(|e| e.to_string())?;
    let storage = if no_keyring {
        "config-file"
    } else {
        "os-keychain"
    };
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": true,
            "appid": appid,
            "secret": wxwright_mp::mask(&secret),
            "storage": storage,
            "verified": verified,
            "nickname": nickname,
            "warning": warning,
        }));
    } else {
        out.ok(&format!(
            "credentials stored ({}).",
            if no_keyring {
                "in config file"
            } else {
                "in OS keychain"
            }
        ));
        if let Some(name) = &nickname {
            out.info(&format!("verified: account {}", name));
        } else if verified {
            out.info("verified: token acquired");
        }
        if let Some(w) = &warning {
            out.warn(&format!("not verified: {}", w));
            if w.contains("40164") {
                out.info("add this machine's public IP to the MP API whitelist, then rerun `wxwright doctor`");
            }
        }
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
    // Zero samples would make the median index `samples[0]` panic and the p95
    // computation divide by zero. Refuse rather than crash.
    if iters == 0 {
        return Err("--iters 必须大于 0（0 次采样没有中位数可算）".into());
    }
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
    // Stage timings run the exact pipeline() sequence (parse -> render ->
    // stats -> normalize -> validate) so a regression can be attributed
    // without a profiler.
    let mut total: Vec<f64> = Vec::with_capacity(iters);
    let mut s_parse: Vec<f64> = Vec::with_capacity(iters);
    let mut s_render: Vec<f64> = Vec::with_capacity(iters);
    let mut s_stats: Vec<f64> = Vec::with_capacity(iters);
    let mut s_norm: Vec<f64> = Vec::with_capacity(iters);
    let mut s_valid: Vec<f64> = Vec::with_capacity(iters);
    let mut html_bytes = 0usize;
    let mut violation_count = 0usize;
    for _ in 0..iters {
        let t0 = std::time::Instant::now();
        let blocks = wxwright_core::parser::parse_markdown(&md);
        let t1 = std::time::Instant::now();
        let img = wxwright_core::img::ImgPipeline::new(
            opts.image_mode,
            opts.base_dir.clone(),
            opts.transport.clone(),
        );
        let rendered = wxwright_core::render::render_document(&blocks, &opts.theme, &img);
        let t2 = std::time::Instant::now();
        let _doc_stats = wxwright_core::ir::count_stats(&blocks);
        let t3 = std::time::Instant::now();
        let normalized = wxwright_core::normalizer::normalize_html(
            &rendered.html,
            wxwright_core::normalizer::NormalizeOptions::default(),
        )
        .map_err(|e| e.to_string())?;
        let t4 = std::time::Instant::now();
        let violations = wxwright_core::validator::validate_html(&normalized.html);
        let t5 = std::time::Instant::now();

        s_parse.push(t1.duration_since(t0).as_secs_f64() * 1000.0);
        s_render.push(t2.duration_since(t1).as_secs_f64() * 1000.0);
        s_stats.push(t3.duration_since(t2).as_secs_f64() * 1000.0);
        s_norm.push(t4.duration_since(t3).as_secs_f64() * 1000.0);
        s_valid.push(t5.duration_since(t4).as_secs_f64() * 1000.0);
        total.push(t5.duration_since(t0).as_secs_f64() * 1000.0);
        html_bytes = html_bytes.max(normalized.html.len());
        violation_count = violation_count.max(violations.len());
    }
    let sorted = |s: &mut Vec<f64>| {
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    };
    for s in [
        &mut total,
        &mut s_parse,
        &mut s_render,
        &mut s_stats,
        &mut s_norm,
        &mut s_valid,
    ] {
        sorted(s);
    }
    let p50 = |s: &[f64]| s[s.len() / 2];
    let p95 = |s: &[f64]| s[(s.len() as f64 * 0.95) as usize % s.len()];
    let within_slo = p50(&total) <= 60.0;
    let build = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    if out.json_mode {
        out.print_json(&serde_json::json!({
            "ok": true,
            "build": build,
            "doc_chars": md.chars().count(),
            "iterations": iters,
            "pipeline_ms_p50": p50(&total),
            "pipeline_ms_p95": p95(&total),
            "stages_ms_p50": {
                "parse": p50(&s_parse),
                "render": p50(&s_render),
                "stats": p50(&s_stats),
                "normalize": p50(&s_norm),
                "validate": p50(&s_valid),
            },
            "html_bytes": html_bytes,
            "violations": violation_count,
            "slo_p2_budget_ms": 60.0,
            "within_slo": within_slo,
            "note": if !within_slo && cfg!(debug_assertions) {
                Some("debug build: the SLO budget applies to release builds")
            } else {
                None
            },
        }));
    } else {
        out.ok(&format!(
            "full chain over {} chars: p50 {:.1}ms, p95 {:.1}ms (SLO budget 60ms, {} build)",
            md.chars().count(),
            p50(&total),
            p95(&total),
            build
        ));
        if !within_slo && cfg!(debug_assertions) {
            out.warn("above the SLO budget; debug builds are expected to be slower - measure a release build");
        }
        out.info(&format!(
            "stages p50 ms: parse {:.1} · render {:.1} · stats {:.1} · normalize {:.1} · validate {:.1}",
            p50(&s_parse),
            p50(&s_render),
            p50(&s_stats),
            p50(&s_norm),
            p50(&s_valid)
        ));
    }
    Ok(0)
}

#[cfg(test)]
mod scaffold_tests {
    use super::*;

    /// `theme new` hands the user a starting point; it must parse as a valid
    /// theme (including the variant blocks) or the first command a new theme
    /// author runs is a failure.
    #[test]
    fn scaffold_parses_and_ships_variants() {
        let toml = theme_scaffold("my-theme");
        let theme =
            wxwright_core::theme::parse_theme(&toml).expect("scaffold must parse as a valid theme");
        assert_eq!(theme.meta.id, "my-theme");
        for key in [
            "quote.hero",
            "quote.hero_leaf",
            "paragraph.lead",
            "paragraph.lead_leaf",
        ] {
            assert!(
                theme.blocks.contains_key(key),
                "scaffold must ship the {key} variant"
            );
        }
    }
}
