//! CLI behavior contract (AGENTS.md §6: no fix ships without evidence).
//!
//! These tests drive the real `wxwright` binary, so they lock what an agent or
//! a CI gate actually observes: exit codes, the JSON document shape, stdin and
//! `--out` handling, frontmatter, the platform registry, and the guard rails
//! that must refuse before touching credentials, the clipboard or the material
//! library.
//!
//! Every run gets a throwaway config root (APPDATA / USERPROFILE / HOME /
//! XDG_CONFIG_HOME - all four, because `util::config_root()` branches per
//! platform, pitfall 52) and MP credential env vars removed, so a test can
//! neither read nor overwrite the developer's real WeChat binding.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

const FAKE_APPID: &str = "wx0000000000000f00";
/// Long enough that `mask()` cannot reveal the whole thing; never appears in
/// output (asserted below).
const FAKE_SECRET: &str = "integration-probe-secret-0123456789-abcd";

fn sandbox_counter() -> &'static AtomicUsize {
    static N: AtomicUsize = AtomicUsize::new(0);
    &N
}

#[derive(Debug)]
struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    /// Stdout must be exactly one JSON document - statuses and diagnostics
    /// belong on stderr (see output.rs).
    fn json(&self) -> Value {
        let trimmed = self.stdout.trim();
        assert!(
            !trimmed.is_empty(),
            "empty stdout (exit {}); stderr was:\n{}",
            self.code,
            self.stderr
        );
        match serde_json::from_str::<Value>(trimmed) {
            Ok(v) => v,
            Err(e) => panic!("stdout is not one JSON document ({e}); got:\n{trimmed}"),
        }
    }

    fn rule_ids(&self) -> Vec<String> {
        let v = self.json();
        let arr = v
            .get("violations")
            .or_else(|| v.get("remaining_violations"))
            .and_then(|x| x.as_array())
            .expect("a violations array");
        arr.iter()
            .map(|x| x["rule_id"].as_str().unwrap_or("").to_string())
            .collect()
    }
}

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Sandbox {
        let n = sandbox_counter().fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "wxwright-cli-{}-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_micros())
                .unwrap_or(0),
            n
        ));
        let s = Sandbox { dir };
        for sub in [
            "work",
            "appdata/wxwright/themes",
            "config/wxwright/themes",
            "home",
        ] {
            std::fs::create_dir_all(s.dir.join(sub))
                .unwrap_or_else(|e| panic!("cannot create {sub}: {e}"));
        }
        s
    }

    fn work(&self) -> PathBuf {
        self.dir.join("work")
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Absolute path string for a file that is deliberately absent.
    fn abs(&self, name: &str) -> String {
        self.path(name).display().to_string()
    }

    /// True when a stray file appeared in the work directory (the process cwd).
    fn stray(&self, name: &str) -> bool {
        self.work().join(name).exists()
    }

    /// Write a fixture into the work directory and return its path string.
    fn write(&self, name: &str, content: &str) -> String {
        self.write_bytes(name, content.as_bytes());
        self.work().join(name).display().to_string()
    }

    fn write_bytes(&self, name: &str, content: &[u8]) -> PathBuf {
        let p = self.work().join(name);
        std::fs::write(&p, content).unwrap_or_else(|e| panic!("cannot write {name}: {e}"));
        p
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.work().join(name))
            .unwrap_or_else(|e| panic!("cannot read fixture {name}: {e}"))
    }

    fn run(&self, args: &[&str]) -> Run {
        self.exec(args, None, &[])
    }

    fn run_with_stdin(&self, args: &[&str], stdin: &str) -> Run {
        self.exec(args, Some(stdin), &[])
    }

    /// A run with extra environment (used for the credential-bearing cases).
    fn run_env(&self, args: &[&str], env: &[(&str, &str)]) -> Run {
        self.exec(args, None, env)
    }

    fn exec(&self, args: &[&str], stdin: Option<&str>, extra: &[(&str, &str)]) -> Run {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_wxwright"));
        cmd.args(args)
            .current_dir(self.work())
            .env("APPDATA", self.path("appdata"))
            .env("USERPROFILE", self.path("home"))
            .env("HOME", self.path("home"))
            .env("XDG_CONFIG_HOME", self.path("config"))
            .env("NO_COLOR", "1")
            .env("WXWRIGHT_LANG", "en")
            // Never inherit a real binding: the CLI reads these before the
            // config file and would then attempt live uploads.
            .env_remove("WXWRIGHT_MP_APPID")
            .env_remove("WXWRIGHT_MP_SECRET")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in extra {
            cmd.env(k, v);
        }
        if stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd.spawn().expect("the wxwright binary must start");
        if let Some(input) = stdin {
            child
                .stdin
                .as_mut()
                .expect("stdin piped")
                .write_all(input.as_bytes())
                .expect("stdin writable");
        }
        drop(child.stdin.take());
        let out = child.wait_with_output().expect("the process must finish");
        Run {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// --------------------------------------------------------------- fixtures --

const CLEAN_MD: &str = "---
title: 合规标题
theme: minimal
---

# 公众号标题

正文段落，包含**加粗**与`行内代码`。

> [!NOTE]
> 提示卡片内容。
";

/// 26 chars: over the Xiaohongshu 20-char title cap, so XHS-1 (warn) fires.
const XHS_LONG_TITLE_MD: &str = "---
title: 这个标题一共二十六个字符用来命中小红书标题提醒
---

正文。
";

/// A title of 75 chars: over the WeChat 64-char cap, so WX-T1 (block) fires.
fn wx_long_title_md() -> String {
    format!(
        "---
title: {}
---

# 正文

内容。
",
        "长标题".repeat(25)
    )
}

const REMOTE_IMG_MD: &str = "---
title: 远端图
---

![远端](https://example.com/a.png)
";

const LOCAL_IMG_MD: &str = "---
title: 本地图
---

![本地](pic.png)
";

const MIXED_IMG_MD: &str = "---
title: 图集
---

![本地](pic.png)

![远端](https://example.com/a.png)
";

/// Passes validate_html untouched (R-2.2 only forbids span[leaf] around blocks).
const CLEAN_HTML: &str = "<section style=\"line-height: 1.6;\"><span leaf=\"1\" style=\"color: #333333; line-height: 1.6;\">ok</span></section>\n";

/// Fixed by the normalizer: R-1.4 (fixed px width) and R-3.1 (font-family).
const DIRTY_HTML: &str = "<section style=\"width: 2000px; font-family: Arial;\"><p style=\"color: #333333\">外部工具的 HTML</p></section>\n";

/// A PNG the image pipeline and `image fit` can actually decode.
fn sample_png() -> Vec<u8> {
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(120, 80, |x, y| {
        image::Rgba([47 + (x % 40) as u8, 108, 234 - (y % 30) as u8, 255])
    }));
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .expect("png encodes");
    out
}

fn masked_ok(run: &Run) {
    assert!(
        !run.stdout.contains(FAKE_SECRET) && !run.stderr.contains(FAKE_SECRET),
        "the secret must never reach stdout/stderr (PRD 5.6 / pitfall 16)"
    );
}

// ------------------------------------------------------------------ tests --

#[test]
fn version_and_usage_errors_follow_the_exit_contract() {
    let s = Sandbox::new("usage");

    let v = s.run(&["--version"]);
    assert_eq!(v.code, 0, "--version must succeed");
    assert!(
        v.stdout.contains(env!("CARGO_PKG_VERSION")),
        "`--version` must report this crate's version, got {:?}",
        v.stdout.trim()
    );

    // clap's own failures are "could not run" => exit 2, same as a bad flag
    // value, so an agent sees one contract for every refusal.
    for args in [
        vec!["frobnicate"],
        vec!["convert"],
        vec!["mcp", "install", "--target", "bogus"],
        vec!["image", "fit", "a.md", "--platform", "xhs"], // --out-dir is required
    ] {
        let r = s.run(&args);
        assert_eq!(r.code, 2, "{args:?} must be a usage error (exit 2)");
        assert!(
            r.stderr.contains("error"),
            "{args:?} must explain the refusal on stderr, got {:?}",
            r.stderr
        );
        assert!(
            r.stdout.trim().is_empty(),
            "{args:?} must not print a report on stdout: {:?}",
            r.stdout
        );
    }

    let help = s.run(&["--help"]);
    assert_eq!(help.code, 0);
    for cmd in [
        "convert",
        "validate",
        "fix",
        "copy",
        "image",
        "draft",
        "publish",
        "theme",
        "doctor",
        "platforms",
        "mcp",
        "agent-card",
        "login",
        "logout",
        "bench",
    ] {
        assert!(
            help.stdout.contains(cmd) || help.stderr.contains(cmd),
            "--help must list `{cmd}`"
        );
    }
}

#[test]
fn validate_reports_a_clean_article_as_one_json_document() {
    let s = Sandbox::new("validate-clean");
    let md = s.write("clean.md", CLEAN_MD);

    let r = s.run(&["--json", "validate", &md]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["compliant"], Value::Bool(true));
    assert_eq!(j["blocking_count"], Value::from(0));
    assert_eq!(j["warnings_count"], Value::from(0));
    assert_eq!(j["platform"], "wechat");
    assert_eq!(j["platform_source"], "default");
    assert_eq!(j["input_format"], "markdown");
    assert_eq!(j["export_kind"], "rich_text_dialect");
    assert_eq!(j["title"], "合规标题");
    assert_eq!(j["violations"], Value::Array(Vec::new()));

    // --verbose and --official-check add stderr lines, never stdout noise.
    let noisy = s.run(&["--json", "--verbose", "validate", &md, "--official-check"]);
    assert_eq!(noisy.code, 0);
    let _ = noisy.json();
    assert!(
        noisy.stderr.contains("validate: input="),
        "--verbose must emit diagnostics on stderr, got {:?}",
        noisy.stderr
    );
}

#[test]
fn validate_reads_crlf_frontmatter_and_its_platform_and_title() {
    let s = Sandbox::new("validate-crlf");
    let md = s.write("crlf.md", &CLEAN_MD.replace("\n", "\r\n"));

    let r = s.run(&["--json", "validate", &md]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    // Regression: strip_frontmatter() used to require a bare "\n---\n", so a
    // library file saved with CRLF silently lost its title and platform.
    assert_eq!(j["title"], "合规标题", "CRLF frontmatter must parse");
    assert_eq!(j["platform"], "wechat");
    assert_eq!(j["platform_source"], "default");

    let zhihu = s.write(
        "crlf-zhihu.md",
        &"---\ntitle: CRLF 标题\nplatform: zhihu\n---\n\n# CRLF\n\n正文。\n".replace("\n", "\r\n"),
    );
    let r = s.run(&["--json", "validate", &zhihu]);
    assert_eq!(r.code, 0);
    let j = r.json();
    assert_eq!(j["platform"], "zhihu");
    assert_eq!(j["platform_source"], "frontmatter");
    assert_eq!(j["title"], "CRLF 标题");
    assert_eq!(j["export_kind"], "markdown");
}

#[test]
fn an_explicit_unknown_platform_refuses_and_a_frontmatter_one_falls_back() {
    let s = Sandbox::new("platform-unknown");
    let md = s.write("clean.md", CLEAN_MD);
    let remote = s.write("remote.md", REMOTE_IMG_MD);

    let r = s.run(&["--json", "validate", &md, "--platform", "nope"]);
    assert_eq!(r.code, 2, "a typo in --platform must not be a warning");
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(false));
    let err = j["error"].as_str().unwrap_or_default().to_string();
    assert!(err.contains("unknown platform"), "{err}");
    assert!(
        err.contains("wechat") && err.contains("linkedin"),
        "the refusal must list the supported ids: {err}"
    );

    let fit = s.run(&[
        "image",
        "fit",
        &remote,
        "--platform",
        "bogus",
        "--out-dir",
        &s.path("fit").display().to_string(),
    ]);
    assert_eq!(fit.code, 2, "image fit validates the platform id too");

    // Frontmatter is author metadata, so an unknown id warns and degrades.
    let typo = s.write(
        "frontmatter-typo.md",
        "---\ntitle: 标题\nplatform: wechart\n---\n\n# 标题\n\n正文。\n",
    );
    let r = s.run(&["--json", "validate", &typo]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["platform"], "wechat");
    assert_eq!(j["platform_known"], Value::Bool(false));
    assert_eq!(j["platform_source"], "frontmatter");
    assert!(
        r.stderr.contains("unknown platform"),
        "the fallback must be announced: {:?}",
        r.stderr
    );
}

#[test]
fn blocking_caption_rules_exit_one_and_strict_gates_warnings() {
    let s = Sandbox::new("validate-severity");

    // XHS-2 (body > 1000 chars) is a block: the note cannot be posted as-is.
    let long = s.write(
        "xhs-long.md",
        &format!("---\ntitle: 短标题\n---\n\n{}\n", "字".repeat(1200)),
    );
    let r = s.run(&["--json", "validate", &long, "--platform", "xhs"]);
    assert_eq!(r.code, 1, "a blocking caption rule must exit 1");
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(false));
    assert_eq!(j["compliant"], Value::Bool(false));
    assert!(j["blocking_count"].as_i64().unwrap_or(0) >= 1);
    assert!(r.rule_ids().iter().any(|id| id == "XHS-2"), "{j}");

    // Warnings alone pass; --strict turns them into a gate failure.
    let warn = s.write("xhs-title.md", XHS_LONG_TITLE_MD);
    let loose = s.run(&["--json", "validate", &warn, "--platform", "xhs"]);
    assert_eq!(loose.code, 0, "stderr:\n{}", loose.stderr);
    let lj = loose.json();
    assert_eq!(lj["ok"], Value::Bool(true));
    assert_eq!(lj["blocking_count"], Value::from(0));
    assert!(lj["warnings_count"].as_i64().unwrap_or(0) >= 1);
    assert!(
        loose.rule_ids().iter().any(|id| id == "XHS-1"),
        "XHS-1 must fire on a 26-char title"
    );

    let strict = s.run(&["--json", "validate", &warn, "--platform", "xhs", "--strict"]);
    assert_eq!(strict.code, 1, "--strict must gate warnings");
    let sj = strict.json();
    assert_eq!(sj["ok"], Value::Bool(false));
    assert_eq!(sj["strict"], Value::Bool(true));

    // WeChat's title cap is a block even though the body renders fine.
    let wx = s.write("wx-long-title.md", &wx_long_title_md());
    let r = s.run(&["--json", "validate", &wx]);
    assert_eq!(r.code, 1);
    assert!(r.rule_ids().iter().any(|id| id == "WX-T1"));
}

#[test]
fn markdown_syntax_findings_warn_but_never_block() {
    let s = Sandbox::new("validate-md");

    let unclosed = s.write("unclosed.md", "# 标题\n\n```rust\nfn main() {}\n");
    let r = s.run(&["--json", "validate", &unclosed]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["compliant"], Value::Bool(true));
    assert_eq!(j["blocking_count"], Value::from(0));
    assert!(j["warnings_count"].as_i64().unwrap_or(0) >= 1);
    assert!(r.rule_ids().iter().any(|id| id == "MD-1"), "{j}");

    let dead = s.write(
        "dead.md",
        "# 标题\n\n![图](https://example.com/a.png)\n\n[空链接]()\n",
    );
    let r = s.run(&["--json", "validate", &dead]);
    assert_eq!(r.code, 0, "a dead link is a finding, not a blocker");
    assert!(r.rule_ids().iter().any(|id| id == "MD-2"));

    // ... but --strict still reports it, which is why CI should use --strict.
    let strict = s.run(&["--json", "validate", &unclosed, "--strict"]);
    assert_eq!(strict.code, 1);
}

#[test]
fn convert_writes_the_dialect_html_and_reports_its_destination() {
    let s = Sandbox::new("convert-wechat");
    let md = s.write("clean.md", CLEAN_MD);
    let out = s.work().join("article.html").display().to_string();

    let r = s.run(&["--json", "convert", &md, "--out", &out]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["platform"], "wechat");
    assert_eq!(j["theme"], "minimal", "frontmatter theme wins over default");
    assert_eq!(j["image_mode"], "inline", "no credentials => no uploads");
    assert_eq!(j["out"], out);
    let html = j["html"].as_str().unwrap_or_default();
    assert!(html.starts_with("<!doctype html>"), "wrapped document");
    assert!(
        html.contains("max-width: 677px"),
        "MP-safe container: {html}"
    );
    assert_eq!(
        std::fs::read_to_string(&out)
            .expect("artifact written")
            .as_str(),
        html,
        "`html` and the --out file must be the same bytes"
    );
    assert_eq!(j["paste_hostile_images"], Value::Array(Vec::new()));

    // A blocking finding must reach the exit code even with --out.
    let long = s.write("long.md", &wx_long_title_md());
    let r = s.run(&["--json", "convert", &long, "--out", "-"]);
    assert_eq!(r.code, 1, "convert must fail on a blocking title rule");
    assert_eq!(r.json()["ok"], Value::Bool(false));
    assert!(
        !s.stray("-"),
        "--out - is stdout, not a file named - in the working directory"
    );

    let badtheme = s.run(&["--json", "convert", &md, "--theme", "nope", "--out", "-"]);
    assert_eq!(badtheme.code, 2, "an unknown theme is \"could not run\"");
    assert!(badtheme.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("built-in themes"));

    let missing = s.run(&["--json", "convert", &s.abs("nowhere.md"), "--out", "-"]);
    assert_eq!(missing.code, 2);
    assert!(missing.json()["error"]
        .as_str()
        .unwrap_or_default()
        .starts_with("cannot read"));
}

#[test]
fn convert_emits_each_platforms_real_artifact() {
    let s = Sandbox::new("convert-platform");
    let md = s.write("clean.md", CLEAN_MD);

    // Xiaohongshu takes a plain-text caption, not styled HTML.
    let r = s.run(&[
        "--json",
        "convert",
        &md,
        "--platform",
        "xhs",
        "--out",
        "cap.txt",
    ]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["export_kind"], "caption");
    assert_eq!(j["platform"], "xhs");
    assert_eq!(
        j["title"], "合规标题",
        "text artifacts must carry the title"
    );
    let text = j["text"].as_str().unwrap_or_default();
    assert!(!text.is_empty());
    assert!(
        !text.contains("<section") && !text.contains("style="),
        "a caption must be plain text: {text}"
    );
    assert_eq!(s.read("cap.txt"), text, "the file is the artifact");

    // Zhihu takes the Markdown through unchanged; stdin must work identically.
    let r = s.run_with_stdin(&["--json", "convert", "-", "--platform", "zhihu"], CLEAN_MD);
    assert_eq!(r.code, 0);
    let j = r.json();
    assert_eq!(j["export_kind"], "markdown");
    assert_eq!(j["out"], Value::Null, "--out - is a null destination");
    assert!(j["text"]
        .as_str()
        .unwrap_or_default()
        .contains("# 公众号标题"));

    // A blocking caption rule fails the export, same contract as validate.
    let long = s.write(
        "xhs-long.md",
        &format!("---\ntitle: 短标题\n---\n\n{}\n", "字".repeat(1200)),
    );
    let r = s.run(&[
        "--json",
        "convert",
        &long,
        "--platform",
        "xhs",
        "--out",
        "-",
    ]);
    assert_eq!(r.code, 1);
    assert_eq!(r.json()["ok"], Value::Bool(false));

    // X weights CJK double; a long Chinese caption cannot be posted.
    let x = s.write(
        "x-long.md",
        &format!("---\ntitle: 标题\n---\n\n{}\n", "字".repeat(200)),
    );
    let r = s.run(&["--json", "convert", &x, "--platform", "x", "--out", "-"]);
    assert_eq!(r.code, 1, "400 weighted chars cannot fit a 280-char post");
    assert!(r.rule_ids().iter().any(|id| id == "X-1"), "{:?}", r.json());
}

#[test]
fn direct_html_is_a_wechat_only_entry() {
    let s = Sandbox::new("convert-html");
    let html = s.write("clean.html", CLEAN_HTML);

    let r = s.run(&["--json", "convert", &html, "--out", "-"]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["input_format"], "html", "the .html extension selects it");
    assert_eq!(j["ok"], Value::Bool(true));
    assert!(j["html"]
        .as_str()
        .unwrap_or_default()
        .contains("<!doctype html>"));

    let xhs = s.run(&[
        "--json",
        "convert",
        &html,
        "--platform",
        "xhs",
        "--out",
        "-",
    ]);
    assert_eq!(xhs.code, 2, "HTML for a caption platform is a usage error");
    assert!(xhs.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("WeChat-only"));

    // stdin auto-detection and an explicit --input-format both work.
    let r = s.run_with_stdin(&["--json", "convert", "-", "--out", "-"], DIRTY_HTML);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["input_format"], "html", "a leading '<' on stdin is HTML");
    assert!(
        !j["fixes"].as_array().unwrap().is_empty(),
        "the normalizer must report the fixes it applied"
    );
    assert!(
        !j["html"].as_str().unwrap_or_default().contains("2000px"),
        "fixed px width must be gone"
    );

    // Markdown that happens to contain HTML stays markdown.
    let md = s.write("mixed.md", "# 标题\n\n<section>x</section>\n");
    let r = s.run(&[
        "--json",
        "convert",
        &md,
        "--input-format",
        "markdown",
        "--out",
        "-",
    ]);
    assert_eq!(r.code, 0);
    assert_eq!(r.json()["input_format"], "markdown");
}

#[test]
fn fix_rewrites_fixable_input_and_still_reports_what_remains() {
    let s = Sandbox::new("fix");

    let dirty = s.write("dirty.html", DIRTY_HTML);
    let r = s.run(&["--json", "fix", &dirty, "--out", "clean.html"]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["input_format"], "html");
    assert_eq!(j["out"], "clean.html");
    assert_eq!(j["blocking_count"], Value::from(0));
    let ids: Vec<&str> = j["fixes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["rule_id"].as_str().unwrap_or_default())
        .collect();
    assert!(ids.contains(&"R-1.4") && ids.contains(&"R-3.1"), "{j}");
    let fixed = s.read("clean.html");
    assert!(
        !fixed.contains("2000px") && !fixed.contains("font-family"),
        "{fixed}"
    );

    // `fix article.md` returns the rendered HTML artifact, not a second step.
    let md = s.write("clean.md", CLEAN_MD);
    let r = s.run(&["--json", "fix", &md, "--out", "from-md.html"]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    assert!(s.read("from-md.html").contains("<section"));

    // A rule no normalizer can fix (the author's title) must stay visible.
    let long = s.write("long.md", &wx_long_title_md());
    let r = s.run(&["--json", "fix", &long, "--out", "long.html"]);
    assert_eq!(r.code, 1, "remaining blocking findings must not be a pass");
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(false));
    assert_eq!(j["blocking_count"], Value::from(1));
    assert!(j["remaining_violations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["rule_id"] == "WX-T1"));

    // `fix --out -` is the stdout destination. The released binary wrote to a
    // file literally named "-" instead of to stdout.
    let r = s.run(&["--json", "fix", &dirty, "--out", "-"]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["out"], Value::Null, "stdout is the destination");
    let html = j["html"].as_str().expect("html field");
    assert!(
        html.contains("<section") && !html.contains("font-family"),
        "{html}"
    );
    assert!(!s.stray("-"), "a file named - must not be created");

    let missing = s.run(&["--json", "fix", &s.abs("nope.html"), "--out", "-"]);
    assert_eq!(missing.code, 2);
}

#[test]
fn copy_dry_run_touches_neither_the_clipboard_nor_the_material_library() {
    let s = Sandbox::new("copy-dry");

    // No credentials: a local image cannot survive a paste, so the dry run
    // says so instead of reporting a false success.
    let md = s.write("local.md", LOCAL_IMG_MD);
    s.write_bytes("pic.png", &sample_png());
    let r = s.run(&["--json", "copy", &md, "--dry-run"]);
    assert_eq!(r.code, 1, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(false));
    assert_eq!(j["dry_run"], Value::Bool(true));
    assert_eq!(j["copied"], Value::Bool(false));
    assert!(j["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("paste-hostile"));
    assert_eq!(j["local_images"], serde_json::json!(["pic.png"]));

    // With credentials the real run would upload; a dry run must not, and the
    // image record proves it: no media_id, no upload warning.
    let r = s.run_env(
        &["--json", "copy", &md, "--dry-run"],
        &[
            ("WXWRIGHT_MP_APPID", FAKE_APPID),
            ("WXWRIGHT_MP_SECRET", FAKE_SECRET),
        ],
    );
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["copied"], Value::Bool(false));
    assert_eq!(
        j["uploads_skipped"],
        Value::Bool(true),
        "a dry run may not upload (iron law 12)"
    );
    let img = &j["report"]["images"][0];
    assert_eq!(img["media_id"], Value::Null, "no upload may happen");
    assert_eq!(img["warning"], Value::Null, "no upload attempt may happen");
    masked_ok(&r);

    // A clean article without images passes the gate.
    let clean = s.write("clean.md", CLEAN_MD);
    let r = s.run(&["--json", "copy", &clean, "--dry-run"]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["platform"], "wechat");
    assert_eq!(j["title"], "合规标题");

    // Blocking violations are refused before anything is written.
    let long = s.write("long.md", &wx_long_title_md());
    let r = s.run(&["--json", "copy", &long, "--dry-run"]);
    assert_eq!(r.code, 1);
    let j = r.json();
    assert_eq!(j["reason"], "blocking violations");
    assert_eq!(j["copied"], Value::Bool(false));

    let missing = s.run(&["--json", "copy", &s.abs("nope.md"), "--dry-run"]);
    assert_eq!(missing.code, 2);
}

#[test]
fn theme_lifecycle_scaffolds_protects_and_validates() {
    let s = Sandbox::new("theme");

    let r = s.run(&["--json", "theme", "list"]);
    assert_eq!(r.code, 0);
    let list = r.json();
    let ids: Vec<&str> = list["themes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap_or_default())
        .collect();
    for builtin in ["minimal", "techblue", "magazine"] {
        assert!(ids.contains(&builtin), "built-in {builtin} listed: {ids:?}");
    }
    assert_eq!(r.json()["ok"], Value::Bool(true));

    let r = s.run(&["--json", "theme", "new", "probe"]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["path"], "probe.toml");
    assert_eq!(j["overwritten"], Value::Bool(false));

    // No silent clobber of work already done.
    let again = s.run(&["--json", "theme", "new", "probe"]);
    assert_eq!(again.code, 2);
    assert!(again.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("--force"));
    let forced = s.run(&["--json", "theme", "new", "probe", "--force"]);
    assert_eq!(forced.code, 0);
    assert_eq!(forced.json()["overwritten"], Value::Bool(true));

    // The scaffold must be usable as-is, or the first command a new theme
    // author runs is a dead end.
    let v = s.run(&["--json", "theme", "validate", "probe.toml"]);
    assert_eq!(v.code, 0, "stderr:\n{}", v.stderr);
    let vj = v.json();
    assert_eq!(vj["ok"], Value::Bool(true));
    assert_eq!(vj["theme"]["id"], "probe");

    // A theme is loadable by id once it lives in the user theme directory.
    // `util::config_root()` branches per platform (APPDATA on Windows,
    // XDG_CONFIG_HOME else HOME/.config on unix), so the fixture has to land in
    // both layouts or this passes on Windows and fails on the Linux/macOS
    // runners (AGENTS.md pitfall 52).
    for dir in ["appdata/wxwright/themes", "config/wxwright/themes"] {
        std::fs::copy(
            s.work().join("probe.toml"),
            s.path(dir).join("fromdir.toml"),
        )
        .unwrap_or_else(|e| panic!("copy into {dir}: {e}"));
    }
    let r = s.run(&[
        "--json",
        "convert",
        &s.write("clean.md", CLEAN_MD),
        "--theme",
        "fromdir",
        "--out",
        "-",
    ]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    assert_eq!(r.json()["theme"], "probe");

    for (args, why) in [
        (
            vec!["theme", "new", "bad name!"],
            "must reject a name that cannot be a file",
        ),
        (
            vec!["theme", "validate", "nope.toml"],
            "must reject a missing file",
        ),
    ] {
        let mut full = vec!["--json"];
        full.extend(args.iter().copied());
        let r = s.run(&full);
        assert_eq!(r.code, 2, "{}: {:?}", why, args);
    }

    let broken = s.write("broken.toml", "[meta]\nid = 123\n");
    let r = s.run(&["--json", "theme", "validate", &broken]);
    assert_eq!(r.code, 2, "a theme that cannot parse cannot be validated");
    assert!(r.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("meta"));
}

#[test]
fn image_fit_fits_local_files_and_treats_skips_as_informational() {
    let s = Sandbox::new("image-fit");
    let out_dir = s.path("fit").display().to_string();

    // One local, one remote: the remote one is skipped, which is not a failure.
    let md = s.write("mixed.md", MIXED_IMG_MD);
    s.write_bytes("pic.png", &sample_png());
    let r = s.run(&[
        "--json",
        "image",
        "fit",
        &md,
        "--platform",
        "xhs",
        "--out-dir",
        &out_dir,
    ]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["platform"], "xhs");
    assert_eq!(j["fitted"], Value::from(1));
    assert_eq!(j["skipped"], Value::from(1));
    assert_eq!(j["failed"], Value::from(0));
    assert_eq!(j["out_dir"], out_dir);
    assert_eq!(j["profile"]["max_width"], 1080);
    let files = std::fs::read_dir(&out_dir)
        .expect("out dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1, "numbered artifact: {files:?}");
    assert!(files[0].starts_with("01-pic."), "{files:?}");

    // Every entry carries its own verdict.
    let entries = j["images"].as_array().unwrap();
    assert_eq!(entries[0]["ok"], Value::Bool(true));
    assert_eq!(entries[0]["source"], "pic.png");
    assert_eq!(
        entries[1]["ok"],
        Value::Bool(true),
        "a skip is not a failure"
    );
    assert!(entries[1]["skipped"]
        .as_str()
        .unwrap_or_default()
        .contains("remote"));

    // An article whose images are all remote is a success with nothing fitted.
    let remote = s.write("remote.md", REMOTE_IMG_MD);
    let r = s.run(&[
        "--json",
        "image",
        "fit",
        &remote,
        "--platform",
        "xhs",
        "--out-dir",
        &out_dir,
    ]);
    assert_eq!(r.code, 0, "all-remote must not fail: {:?}", r.json());
    assert_eq!(r.json()["fitted"], Value::from(0));
    assert_eq!(r.json()["skipped"], Value::from(1));

    // A local file that cannot be read is a real failure.
    let missing = s.write("missing.md", "---\ntitle: 缺图\n---\n\n![缺](nope.png)\n");
    let r = s.run(&[
        "--json",
        "image",
        "fit",
        &missing,
        "--platform",
        "xhs",
        "--out-dir",
        &out_dir,
    ]);
    assert_eq!(r.code, 1);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(false));
    assert_eq!(j["failed"], Value::from(1));
    assert!(j["images"][0]["error"]
        .as_str()
        .unwrap_or_default()
        .starts_with("cannot read"));

    // Refusals: unknown platform, HTML input, an article with no images.
    let no_img = s.write("no-img.md", "---\ntitle: 没图\n---\n\n正文。\n");
    for (args, want) in [
        (
            vec![
                "--json",
                "image",
                "fit",
                &md,
                "--platform",
                "bogus",
                "--out-dir",
                &out_dir,
            ],
            "unknown platform",
        ),
        (
            vec![
                "--json",
                "image",
                "fit",
                &s.write("clean.html", CLEAN_HTML),
                "--platform",
                "xhs",
                "--out-dir",
                &out_dir,
            ],
            "Markdown image references",
        ),
        (
            vec![
                "--json",
                "image",
                "fit",
                &no_img,
                "--platform",
                "xhs",
                "--out-dir",
                &out_dir,
            ],
            "no images",
        ),
    ] {
        let r = s.run(&args);
        assert_eq!(r.code, 2, "{args:?} must refuse");
        assert!(
            r.json()["error"]
                .as_str()
                .unwrap_or_default()
                .contains(want),
            "{want}: {:?}",
            r.json()
        );
    }

    // stdin resolves images relative to the working directory.
    let r = s.run_with_stdin(
        &[
            "--json",
            "image",
            "fit",
            "-",
            "--platform",
            "wechat",
            "--out-dir",
            &out_dir,
        ],
        LOCAL_IMG_MD,
    );
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    assert_eq!(r.json()["fitted"], Value::from(1));
}

#[test]
fn image_upload_drafts_and_publishing_refuse_before_touching_the_account() {
    let s = Sandbox::new("publish-guards");

    // Uploading needs credentials; without them the answer is "could not run".
    s.write_bytes("pic.png", &sample_png());
    let r = s.run(&["--json", "image", "upload", "pic.png"]);
    assert_eq!(r.code, 2);
    assert!(r.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("wxwright login"));

    // Irreversible operations are refused before the credential lookup, so a
    // missing binding never masks "rerun with --yes".
    let del = s.run(&["--json", "draft", "delete", "--media-id", "some-media-id"]);
    assert_eq!(del.code, 2);
    assert!(del.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("irreversible"));

    let pub_no = s.run(&["--json", "publish", "some-draft-id"]);
    assert_eq!(pub_no.code, 2);
    let msg = pub_no.json()["error"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(msg.contains("--yes"), "{msg}");
    assert!(
        msg.contains("freepublish") && msg.contains("群发"),
        "the refusal must name what it is NOT: {msg}"
    );

    // Everything that does need an account says so with the repair command.
    for args in [
        vec!["--json", "draft", "list"],
        vec![
            "--json",
            "draft",
            "create",
            "--file",
            &s.write("clean.md", CLEAN_MD),
            "--title",
            "标题",
        ],
        vec![
            "--json",
            "draft",
            "update",
            "--media-id",
            "m",
            "--file",
            &s.write("clean.md", CLEAN_MD),
        ],
    ] {
        let r = s.run(&args);
        assert_eq!(r.code, 2, "{args:?}");
        assert!(r.json()["error"]
            .as_str()
            .unwrap_or_default()
            .contains("no WeChat MP credentials"));
    }
}

#[test]
fn login_accepts_a_stdin_secret_and_never_echoes_it() {
    let s = Sandbox::new("login");

    let both = s.run(&[
        "--json",
        "login",
        "--appid",
        FAKE_APPID,
        "--secret",
        "inline",
        "--secret-stdin",
    ]);
    assert_eq!(
        both.code, 2,
        "--secret and --secret-stdin are mutually exclusive"
    );
    assert!(both.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("not both"));

    let empty = s.run(&[
        "--json",
        "login",
        "--appid",
        "",
        "--secret",
        "whatever",
        "--no-verify",
        "--no-keyring",
    ]);
    assert_eq!(empty.code, 2);
    assert!(empty.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("--appid"));

    let none = s.run(&[
        "--json",
        "login",
        "--appid",
        FAKE_APPID,
        "--no-verify",
        "--no-keyring",
    ]);
    assert_eq!(none.code, 2);
    assert!(none.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("no secret"));

    // The recommended path: secret on stdin, no live check, no keychain.
    let r = s.run_with_stdin(
        &[
            "--json",
            "login",
            "--appid",
            FAKE_APPID,
            "--secret-stdin",
            "--no-verify",
            "--no-keyring",
        ],
        &format!("{FAKE_SECRET}\n"),
    );
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["appid"], FAKE_APPID);
    assert_eq!(j["storage"], "config-file");
    assert_eq!(j["verified"], Value::Bool(false));
    assert!(
        j["secret"].as_str().unwrap_or_default().contains("****"),
        "{j}"
    );
    masked_ok(&r);

    // doctor then reports presence only, still masked.
    let d = s.run(&["--json", "doctor"]);
    let report = d.json();
    let creds = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check"] == "credentials")
        .expect("a credentials check")
        .clone();
    assert_eq!(creds["status"], "ok");
    assert_eq!(creds["detail"]["verified"], Value::Bool(false));
    assert!(creds["detail"]["note"]
        .as_str()
        .unwrap_or_default()
        .contains("--verify"));
    assert!(!d.stdout.contains(FAKE_SECRET) && !d.stderr.contains(FAKE_SECRET));

    // Unbind is idempotent and leaves no credentials behind.
    let out1 = s.run(&["--json", "logout"]);
    assert_eq!(out1.code, 0);
    assert_eq!(out1.json()["ok"], Value::Bool(true));
    assert_eq!(
        s.run(&["--json", "logout"]).code,
        0,
        "logout must be idempotent"
    );
    let d = s.run(&["--json", "doctor"]);
    let report = d.json();
    let creds = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check"] == "credentials")
        .expect("a credentials check")
        .clone();
    assert_eq!(
        creds["status"], "warn",
        "after logout doctor must say the binding is gone"
    );
    assert!(creds["detail"]["hint"]
        .as_str()
        .unwrap_or_default()
        .contains("wxwright login"));
}

#[test]
fn doctor_checks_the_environment_and_states_how_deep_it_looked() {
    let s = Sandbox::new("doctor");
    let r = s.run(&["--json", "doctor"]);
    // doctor is the CI/SCP preflight gate: exit 1 only for errors, or for
    // warnings under --strict.
    let j = r.json();
    assert!(
        j["checks"].is_array(),
        "stdout must be the check list: {:?}",
        r.stdout
    );
    let names: Vec<&str> = j["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["check"].as_str().unwrap_or_default())
        .collect();
    for want in [
        "version",
        "config_file",
        "credentials",
        "clipboard",
        "themes",
        "state_dir",
        "ai_provider",
    ] {
        assert!(names.contains(&want), "missing check {want}: {names:?}");
    }
    let state = j["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check"] == "state_dir")
        .expect("state_dir");
    assert_eq!(
        state["detail"]["writable"],
        Value::Bool(true),
        "the sandboxed state dir must be writable"
    );
    assert_eq!(j["strict"], Value::Bool(false));

    // Without credentials --verify has nothing to check: warn, not a lie.
    let v = s.run(&["--json", "doctor", "--verify"]);
    assert_eq!(v.code, r.code, "--verify must not invent a failure");
    let report = v.json();
    let creds = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check"] == "credentials")
        .expect("credentials");
    assert_eq!(creds["status"], "warn");
    assert_eq!(
        creds["detail"].get("verified"),
        None,
        "a warn without credentials must not claim a verification"
    );

    // The strict gate reports what it is gating on.
    let strict = s.run(&["--json", "doctor", "--strict"]);
    assert_eq!(strict.json()["strict"], Value::Bool(true));
    assert!(strict.code == 0 || strict.code == 1, "{strict:?}");
}

#[test]
fn platforms_lists_the_registry_and_agent_card_locks_the_surface() {
    let s = Sandbox::new("registry");

    let r = s.run(&["--json", "platforms"]);
    assert_eq!(r.code, 0);
    let list = r.json();
    let arr = list.as_array().expect("a platform array");
    assert_eq!(arr.len(), 7, "the registry is 7 platforms: {arr:?}");
    let ids: Vec<&str> = arr
        .iter()
        .map(|p| p["id"].as_str().unwrap_or_default())
        .collect();
    for want in [
        "wechat",
        "xhs",
        "zhihu",
        "meta",
        "instagram",
        "x",
        "linkedin",
    ] {
        assert!(ids.contains(&want), "{want} registered: {ids:?}");
    }
    for p in arr {
        assert!(
            p["export_kind"].is_string(),
            "each descriptor declares its artifact: {p}"
        );
    }

    let card = s.run(&["--json", "agent-card"]);
    assert_eq!(card.code, 0);
    let j = card.json();
    assert_eq!(j["name"], "wxwright");
    assert_eq!(j["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(j["exit_codes"].as_object().unwrap().len(), 3);
    assert_eq!(j["mcp"]["tools"].as_array().unwrap().len(), 12);
    assert_eq!(j["cli"].as_array().unwrap().len(), 20);

    // The Markdown form is what a human pastes into a system prompt; under a
    // pipe it needs --md, otherwise it would be JSON.
    let md = s.run(&["--json", "agent-card", "--md"]);
    assert_eq!(md.code, 0);
    assert!(
        md.stdout.starts_with("# wxwright Agent Card"),
        "{:?}",
        &md.stdout[..40]
    );
    for flag in [
        "--platform",
        "--secret-stdin",
        "--dry-run",
        "--verify",
        "image fit",
    ] {
        assert!(md.stdout.contains(flag), "the card must document {flag}");
    }
    assert!(
        md.stdout.contains("freepublish") && !md.stdout.contains("群发 (mass send); HUMAN-ONLY"),
        "publish is 发布 (freepublish), not 群发 - the card must not claim otherwise"
    );
}

#[test]
fn bench_refuses_an_impossible_sample_and_states_which_build_ran() {
    let s = Sandbox::new("bench");

    let zero = s.run(&["--json", "bench", "--iters", "0"]);
    assert_eq!(zero.code, 2, "--iters 0 has no median to report");
    assert_eq!(zero.json()["ok"], Value::Bool(false));

    let r = s.run(&["--json", "bench", "--iters", "1"]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["iterations"], Value::from(1));
    assert_eq!(j["slo_p2_budget_ms"].as_f64(), Some(60.0));
    // A debug build cannot hit the release SLO; the output has to say so
    // instead of letting a reader conclude the engine regressed.
    let build = j["build"].as_str().unwrap_or_default();
    assert!(build == "debug" || build == "release", "{j}");
    if build == "debug" {
        assert!(
            j["note"].is_string(),
            "a debug result must be annotated: {j}"
        );
    }
}

#[test]
fn mcp_install_writes_only_the_requested_server_and_backups_first() {
    let s = Sandbox::new("mcp-install");
    let cursor_dir = s.path("home/.cursor");
    std::fs::create_dir_all(&cursor_dir).expect("cursor dir");
    let config = cursor_dir.join("mcp.json");
    std::fs::write(
        &config,
        serde_json::json!({ "mcpServers": { "someone-else": { "command": "echo" } } }).to_string(),
    )
    .expect("pre-existing client config");

    let r = s.run(&["--json", "mcp", "install", "--target", "cursor"]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(true));
    assert_eq!(j["target"], "cursor");
    assert_eq!(j["key"], "mcpServers");
    assert_eq!(j["created"], Value::Bool(false), "the file already existed");
    assert_eq!(j["backup_created"], Value::Bool(true));
    assert!(Path::new(&j["config_path"].as_str().unwrap()).ends_with("mcp.json"));

    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(&config).expect("config written"))
            .expect("config parses");
    let servers = written["mcpServers"].as_object().expect("servers");
    assert!(servers.contains_key("wxwright"), "{servers:?}");
    assert!(
        servers.contains_key("someone-else"),
        "installing must not delete another MCP server: {servers:?}"
    );
    assert!(
        cursor_dir.join("mcp.json.bak").exists(),
        "a .bak must be kept"
    );

    // The server itself answers the pinned 12-tool surface over stdio.
    let handshake = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n",
    );
    let r = s.run_with_stdin(&["--json", "mcp", "serve"], handshake);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    let lines: Vec<&str> = r.stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(lines.len() >= 2, "two responses expected: {lines:?}");
    let tools: Value = serde_json::from_str(lines[lines.len() - 1]).expect("tools/list json");
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 12);
}

#[test]
fn unreadable_input_and_unwritable_output_are_refusals_not_failures() {
    let s = Sandbox::new("io-errors");

    let r = s.run(&["--json", "validate", &s.abs("nowhere.md")]);
    assert_eq!(r.code, 2);
    assert!(r.json()["error"]
        .as_str()
        .unwrap_or_default()
        .contains("cannot read"));

    let md = s.write("clean.md", CLEAN_MD);
    let bad_out = s
        .path("missing-dir/deep/article.html")
        .display()
        .to_string();
    let r = s.run(&["--json", "convert", &md, "--out", &bad_out]);
    assert_eq!(
        r.code, 2,
        "a failing artifact write must not look like success"
    );
    // Regression: the report used to be printed first, so stdout carried an
    // `ok: true` document *and* the error document - and the first one lied.
    let j = r.json();
    assert_eq!(j["ok"], Value::Bool(false), "{j}");
    assert!(
        j.get("html").is_none(),
        "the success report must not be printed at all"
    );
    assert!(j["error"]
        .as_str()
        .unwrap_or_default()
        .contains("cannot write"));
    assert!(
        !Path::new(&bad_out).exists(),
        "no partial artifact may be reported as written"
    );

    // Frontmatter format: html is the same entry as an .html extension.
    let htmlish = s.write(
        "front.html.md",
        "---\ntitle: 直写 HTML\nformat: html\n---\n\n<section style=\"line-height: 1.6;\"><span leaf=\"1\" style=\"color: #333333; line-height: 1.6;\">ok</span></section>\n",
    );
    let r = s.run(&["--json", "validate", &htmlish]);
    assert_eq!(r.code, 0, "stderr:\n{}", r.stderr);
    assert_eq!(
        r.json()["input_format"],
        "html",
        "frontmatter format is honoured"
    );
    let r = s.run(&[
        "--json",
        "convert",
        &htmlish,
        "--platform",
        "xhs",
        "--out",
        "-",
    ]);
    assert_eq!(r.code, 2, "HTML stays WeChat-only even from frontmatter");
}
