//! Platform-aware audit + one-click AI repair.
//!
//! The audit funnels every platform through the *same* validator the rest of
//! the product uses (dialect pipeline, direct-HTML pipeline, or caption rule
//! table), so the repair prompt, the before/after report and the editor's
//! rule chip can never disagree. Repair asks the model for strict JSON, then
//! re-runs the audit and feeds residual violations back for a second round.

use wxwright_core::img::ImageMode;
use wxwright_core::platform::{self, ExportKind};
use wxwright_core::validator::Violation;
use wxwright_core::ConvertOptions;

/// Articles longer than this cannot be repaired in one JSON reply without a
/// truncated answer; the UI tells the user to split the piece instead.
pub const MAX_REPAIR_CHARS: usize = 12_000;

pub struct Audit {
    pub platform: String,
    pub mode: &'static str,
    /// Content without frontmatter - what the model rewrites.
    pub body: String,
    /// Exact frontmatter prefix (including the closing fence), preserved
    /// verbatim across a repair.
    pub front: String,
    pub caption: String,
    pub images: usize,
    pub violations: Vec<Violation>,
}

pub struct FixOutcome {
    pub title: String,
    pub content: String,
    pub before: Vec<Violation>,
    pub after: Vec<Violation>,
    pub rounds: u32,
    pub changed: bool,
}

/// Validate one article for a platform. `format` is `html` for the direct-HTML
/// entry and `markdown` otherwise.
pub fn audit(
    platform: &str,
    title: &str,
    markdown: &str,
    format: &str,
    theme_id: &str,
) -> Result<Audit, String> {
    let spec = platform::get_platform(platform);
    let (front, body) = split_frontmatter(markdown);
    let is_html = format.eq_ignore_ascii_case("html");

    if is_html {
        let out = wxwright_core::pipeline_html(&body).map_err(|e| e.to_string())?;
        let mut violations = out.violations;
        violations.extend(platform::validate_platform_title(spec.id, title));
        return Ok(Audit {
            platform: spec.id.to_string(),
            mode: "html",
            body,
            front,
            caption: String::new(),
            images: 0,
            violations,
        });
    }

    match spec.export_kind {
        ExportKind::Caption => {
            let doc = wxwright_core::parser::parse_markdown(&body);
            let caption = platform::render_caption(&doc, Some(title));
            let images = wxwright_core::ir::count_stats(&doc).images;
            let mut violations =
                platform::validate_platform_caption(spec.id, title, &caption, images);
            violations.extend(wxwright_core::validator::validate_markdown(&body));
            Ok(Audit {
                platform: spec.id.to_string(),
                mode: "caption",
                body,
                front,
                caption,
                images,
                violations,
            })
        }
        ExportKind::RichTextDialect | ExportKind::Markdown => {
            let theme = load_theme(theme_id);
            let opts = ConvertOptions {
                theme,
                image_mode: ImageMode::Inline,
                base_dir: None,
                transport: None,
            };
            let mut out = wxwright_core::pipeline(&body, &opts).map_err(|e| e.to_string())?;
            out.violations
                .extend(wxwright_core::validator::validate_markdown(&body));
            out.violations
                .extend(platform::validate_platform_title(spec.id, title));
            Ok(Audit {
                platform: spec.id.to_string(),
                mode: "dialect",
                body,
                front,
                caption: String::new(),
                images: out.stats.images,
                violations: out.violations,
            })
        }
    }
}

fn load_theme(id: &str) -> wxwright_core::theme::Theme {
    wxwright_core::theme::load_theme(id).unwrap_or_else(|_| {
        wxwright_core::theme::load_builtin_theme("minimal").expect("builtin minimal")
    })
}

/// Split frontmatter from the body while keeping the exact original prefix.
/// `strip_frontmatter` reports the parsed keys and trimmed body; reconstructing
/// the prefix from those keys would rewrite quotes and ordering.
fn split_frontmatter(markdown: &str) -> (String, String) {
    let (pairs, body) = wxwright_core::util::strip_frontmatter(markdown);
    if pairs.is_empty() {
        return (String::new(), markdown.to_string());
    }
    match markdown.rfind(&body) {
        Some(idx) => (markdown[..idx].to_string(), body),
        None => (String::new(), markdown.to_string()),
    }
}

fn reattach(front: &str, body: &str) -> String {
    let body = body.trim();
    if front.trim().is_empty() {
        return format!("{}\n", body);
    }
    let mut out = String::new();
    out.push_str(front);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(body);
    out.push('\n');
    out
}

/// The model's JSON reply, normalized.
struct FixReply {
    title: Option<String>,
    content: String,
}

fn strip_outer_fence(raw: &str) -> String {
    let t = raw.trim();
    let Some(rest) = t.strip_prefix("```") else {
        return t.to_string();
    };
    let Some(nl) = rest.find('\n') else {
        return t.to_string();
    };
    let body = &rest[nl + 1..];
    let body = body.trim_end();
    body.strip_suffix("```").unwrap_or(body).trim().to_string()
}

/// Extract the first balanced JSON object. String contents (including braces
/// and escaped quotes) are skipped, so a caption containing `}` survives.
fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let bytes = raw.as_bytes();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return raw.get(start..=i);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_reply(raw: &str) -> Result<FixReply, String> {
    let cleaned = strip_outer_fence(raw);
    let json = extract_json_object(&cleaned).ok_or_else(|| {
        format!(
            "AI 未返回 JSON 修复结果：{}",
            wxwright_core::util::truncate(raw, 200)
        )
    })?;
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("AI 修复结果不是合法 JSON：{e}"))?;
    let content = v
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if content.is_empty() {
        return Err("AI 修复结果缺少 content".into());
    }
    let title = v
        .get("title")
        .and_then(|t| t.as_str())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string);
    Ok(FixReply { title, content })
}

fn system_prompt(lang: &str, format: &str) -> String {
    let en = lang.to_ascii_lowercase().starts_with("en");
    if format.eq_ignore_ascii_case("html") {
        if en {
            "You are a senior social-media editor and layout engineer. Repair the article's platform-compliance problems without changing the author's facts, opinion, tone or structure. Reply with ONE JSON object only: {\"title\":\"...\",\"content\":\"...\"}. content must be an HTML fragment (inline styles only) with no frontmatter and no code fence around the whole document. Keep every existing image reference; do not invent images or links. Output no explanation."
                .to_string()
        } else {
            "你是资深社交媒体编辑与排版工程师。修复文章的平台合规问题，不改变作者的事实、观点、语气与结构。只返回一个 JSON 对象：{\"title\":\"...\",\"content\":\"...\"}。content 必须是 HTML 片段（只用内联样式），不含 frontmatter，也不要用代码围栏包裹整篇。保留所有已有图片引用，不要虚构图片或链接。不要任何解释文字。"
                .to_string()
        }
    } else if en {
        "You are a senior social-media editor. Repair the article's platform-compliance problems without changing the author's facts, opinion, tone or structure. Reply with ONE JSON object only: {\"title\":\"...\",\"content\":\"...\"}. content must be Markdown source with no frontmatter and no code fence around the whole document. Keep every image reference and paragraph order; do not invent images or links. Output no explanation."
            .to_string()
    } else {
        "你是资深社交媒体编辑。修复文章的平台合规问题，不改变作者的事实、观点、语气与结构。只返回一个 JSON 对象：{\"title\":\"...\",\"content\":\"...\"}。content 必须是 Markdown 源文，不含 frontmatter，也不要用代码围栏包裹整篇。保留所有图片引用与段落顺序，不要虚构图片或链接。不要任何解释文字。"
            .to_string()
    }
}

fn user_prompt(
    lang: &str,
    kind: &str,
    title: &str,
    current: &str,
    caption: Option<&str>,
    violations: &[Violation],
    format: &str,
) -> String {
    let en = lang.to_ascii_lowercase().starts_with("en");
    let is_html = format.eq_ignore_ascii_case("html");
    let mut out = String::new();
    out.push_str(&platform::agent_brief(kind, lang));
    out.push('\n');
    if is_html {
        out.push_str(if en {
            "Note: this is a direct-HTML article. The body is an HTML fragment; the Markdown writing guidance in the brief does not apply. Return an HTML fragment.\n\n"
        } else {
            "注意：本文是直接 HTML 文章，正文是 HTML 片段；简报里的 Markdown 写作指引不适用，返回内容必须是 HTML 片段。\n\n"
        });
    }

    out.push_str(if en {
        "## Violations to eliminate (machine-checked; all must be gone)\n"
    } else {
        "## 必须消除的违规（校验器逐条检查）\n"
    });
    for (i, v) in violations.iter().enumerate() {
        out.push_str(&format!(
            "{}. [{}] {} {}\n",
            i + 1,
            v.severity,
            v.rule_id,
            v.message
        ));
    }
    out.push('\n');

    out.push_str(if en {
        "## Current title\n"
    } else {
        "## 当前标题\n"
    });
    out.push_str(if title.trim().is_empty() {
        "(empty)"
    } else {
        title.trim()
    });
    out.push_str("\n\n");

    if let Some(cap) = caption {
        out.push_str(if en {
            "## Caption readers actually see (this is what the length rules measure)\n"
        } else {
            "## 平台实际展示的文案（长度规则按这段文本计算）\n"
        });
        out.push_str(cap);
        out.push_str("\n\n");
    }

    out.push_str(if en {
        if is_html {
            "## Article to rewrite (HTML fragment)\n<<<ARTICLE\n"
        } else {
            "## Article to rewrite (Markdown source)\n<<<ARTICLE\n"
        }
    } else if is_html {
        "## 待修复正文（HTML 片段）\n<<<ARTICLE\n"
    } else {
        "## 待修复正文（Markdown 源文）\n<<<ARTICLE\n"
    });
    out.push_str(current);
    out.push_str("\nARTICLE>>>\n\n");

    out.push_str(if en {
        "## Task\nFix every violation above. For a length violation, cut redundancy instead of deleting whole sections. For a title violation, shorten the title. Do not add hashtags that misrepresent the content. Return the JSON object now."
    } else {
        "## 任务\n逐条消除上面的违规。长度类违规通过删减冗余表达修复，不要整段删除；标题类违规直接缩短标题；补话题标签时不得与内容不符。现在返回 JSON 对象。"
    });
    out
}

fn score(v: &[Violation]) -> (usize, usize) {
    let blocks = v.iter().filter(|x| x.is_block()).count();
    (blocks, v.len())
}

fn outcome(
    title: String,
    front: &str,
    body: &str,
    before: Vec<Violation>,
    after: Vec<Violation>,
    rounds: u32,
    changed: bool,
) -> FixOutcome {
    FixOutcome {
        content: if changed {
            reattach(front, body)
        } else {
            String::new()
        },
        title,
        before,
        after,
        rounds,
        changed,
    }
}

/// One-click repair: audit, ask the model for strict JSON, re-audit, then one
/// feedback round when violations remain. Applies nothing when no candidate
/// improves on the original.
pub fn fix_article(
    platform: &str,
    title: &str,
    markdown: &str,
    format: &str,
    theme_id: &str,
    lang: &str,
) -> Result<FixOutcome, String> {
    let before = audit(platform, title, markdown, format, theme_id)?;
    if before.violations.is_empty() {
        return Ok(outcome(
            title.to_string(),
            &before.front,
            &before.body,
            Vec::new(),
            Vec::new(),
            0,
            false,
        ));
    }
    let body_chars = before.body.chars().count();
    if body_chars > MAX_REPAIR_CHARS {
        return Err(format!(
            "文章正文 {body_chars} 字，超过一键修复的 {MAX_REPAIR_CHARS} 字上限；请拆分文章或手动删减后重试"
        ));
    }

    let system = system_prompt(lang, format);
    let mut cur_title = title.to_string();
    let mut cur_body = before.body.clone();
    let mut cur_caption = (!before.caption.is_empty()).then(|| before.caption.clone());
    let mut cur_violations = before.violations.clone();
    let mut best: Option<((usize, usize), FixOutcome)> = None;

    for round in 1..=2u32 {
        let user = user_prompt(
            lang,
            &before.platform,
            &cur_title,
            &cur_body,
            cur_caption.as_deref(),
            &cur_violations,
            format,
        );
        let raw = crate::ai::complete(&system, &user, crate::ai::COMPLETE_BUDGET_CAP, 0.15)?;
        let reply = parse_reply(&raw)?;
        let next_title = reply
            .title
            .unwrap_or_else(|| cur_title.clone())
            .chars()
            .take(200)
            .collect::<String>();
        // Models occasionally wrap the body in frontmatter despite the
        // instruction; the original frontmatter is the one that wins.
        let (_, next_body) = split_frontmatter(&reply.content);
        let after = audit(
            &before.platform,
            &next_title,
            &reattach(&before.front, &next_body),
            format,
            theme_id,
        )?;
        let candidate = outcome(
            next_title.clone(),
            &before.front,
            &next_body,
            before.violations.clone(),
            after.violations.clone(),
            round,
            true,
        );
        let candidate_score = score(&after.violations);
        let is_better = best
            .as_ref()
            .map(|(s, _)| candidate_score < *s)
            .unwrap_or(candidate_score < score(&before.violations));

        if after.violations.is_empty() {
            return Ok(candidate);
        }
        if is_better {
            best = Some((candidate_score, candidate));
        }
        cur_title = next_title;
        cur_body = next_body;
        cur_caption = (!after.caption.is_empty()).then(|| after.caption.clone());
        cur_violations = after.violations;
    }

    match best {
        Some((_, o)) => Ok(o),
        None => Err(format!(
            "AI 两轮修复后违规没有减少（原 {} 项）。请检查 AI Provider 或手动修改后重试",
            before.violations.len()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_survives_a_repair_round_trip() {
        let md = "---\ntitle: \"原标题\"\ntheme: minimal\nplatform: xhs\n---\n\n正文\n";
        let (front, body) = split_frontmatter(md);
        assert_eq!(
            front,
            "---\ntitle: \"原标题\"\ntheme: minimal\nplatform: xhs\n---\n\n"
        );
        assert_eq!(body, "正文\n");
        let joined = reattach(&front, "改后的正文");
        assert!(joined.starts_with("---\ntitle: \"原标题\""));
        assert!(joined.ends_with("改后的正文\n"));
        let (front2, body2) = split_frontmatter(&joined);
        assert_eq!(front2, front);
        assert_eq!(body2.trim(), "改后的正文");
    }

    #[test]
    fn audit_uses_the_platform_rule_table() {
        let a = audit("xhs", "短标题", &"字".repeat(1100), "markdown", "minimal").unwrap();
        assert_eq!(a.mode, "caption");
        assert!(
            a.violations
                .iter()
                .any(|v| v.rule_id == "XHS-2" && v.is_block()),
            "expected XHS-2, got {:?}",
            a.violations
        );
        // Frontmatter is not part of the measured caption.
        let with_fm = format!("---\ntitle: 短标题\n---\n\n{}", "字".repeat(1100));
        let b = audit("xhs", "短标题", &with_fm, "markdown", "minimal").unwrap();
        assert!(!b.front.is_empty());
        assert!(b.violations.iter().any(|v| v.rule_id == "XHS-2"));
    }

    #[test]
    fn weighted_x_limit_is_audited() {
        let a = audit("x", "t", &"字".repeat(200), "markdown", "minimal").unwrap();
        assert!(a
            .violations
            .iter()
            .any(|v| v.rule_id == "X-1" && v.is_block()));
    }

    #[test]
    fn reply_parsing_tolerates_fences_and_braces_in_content() {
        let raw =
            "```json\n{\"title\":\"新标题\",\"content\":\"正文里有一个 } 和 \\\"引号\\\"\"}\n```";
        let r = parse_reply(raw).unwrap();
        assert_eq!(r.title.as_deref(), Some("新标题"));
        assert_eq!(r.content, "正文里有一个 } 和 \"引号\"");

        let verbose = "好的，修复如下：\n{\"content\":\"正文\"}\n希望有帮助";
        assert_eq!(parse_reply(verbose).unwrap().content, "正文");
    }

    #[test]
    fn broken_reply_is_an_error_not_silent_application() {
        assert!(parse_reply("我改好了，但没有 JSON").is_err());
        assert!(parse_reply("{\"title\":\"只有标题\"}").is_err());
    }

    #[test]
    fn repair_prompt_carries_the_machine_limits() {
        let v = vec![Violation {
            rule_id: "XHS-2".into(),
            severity: "block".into(),
            message: "caption is 1200 chars; Xiaohongshu notes cap at 1000".into(),
            node: String::new(),
            fixable: false,
        }];
        let p = user_prompt("zh", "xhs", "标题", "正文", None, &v, "markdown");
        assert!(p.contains("小红书"));
        assert!(p.contains("1000"));
        assert!(p.contains("XHS-2"));
        assert!(p.contains("<<<ARTICLE"));
    }

    #[test]
    fn html_repair_prompt_asks_for_an_html_fragment() {
        let p = user_prompt("zh", "wechat", "标题", "<p>正文</p>", None, &[], "html");
        assert!(p.contains("待修复正文（HTML 片段）"));
        assert!(p.contains("简报里的 Markdown 写作指引不适用"));
    }

    #[test]
    fn no_violation_means_no_model_call() {
        let out = fix_article(
            "wechat",
            "标题",
            "# 标题\n\n正文\n",
            "markdown",
            "minimal",
            "zh",
        )
        .expect("clean article");
        assert!(!out.changed);
        assert!(out.before.is_empty());
    }
}
