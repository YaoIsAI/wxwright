//! Platform registry: the "one engine, many dialects" seam (PRD §16).
//!
//! Each social platform gets a descriptor covering what the engine and the
//! GUI need to adapt: whether dialect rich text can be pasted at all, whether
//! the primary flow is an image-note set (Xiaohongshu), what preset canvas
//! sizes the poster/size studios should offer, and which export adapters are
//! actually wired. Descriptors are data, not behaviour — renderers and rule
//! tables stay with the dialect implementations that consume them.

/// One social-media platform the engine can target.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlatformSpec {
    /// Stable id used in settings/CLI (`wechat`, `xhs`, ...).
    pub id: &'static str,
    /// Chinese display name.
    pub name_zh: &'static str,
    /// English display name.
    pub name_en: &'static str,
    /// Dialect rich text (styled HTML) can be pasted into the platform's
    /// editor; false means the primary output is images and/or plain text.
    pub rich_text: bool,
    /// Primary flow is an image-note set (cover + content images).
    pub image_note: bool,
    /// A publishing API adapter is implemented (draft push or equivalent).
    pub api_publish: bool,
    /// Poster / size-studio presets: (label_zh, width, height).
    pub presets: &'static [(&'static str, u32, u32)],
    /// Honest capability note (surfaced in the GUI switcher tooltip).
    pub note: &'static str,
}

pub const WECHAT: PlatformSpec = PlatformSpec {
    id: "wechat",
    name_zh: "微信公众号",
    name_en: "WeChat MP",
    rich_text: true,
    image_note: false,
    api_publish: true,
    presets: &[
        ("头图 1080×460（2.35:1）", 1080, 460),
        ("次图 1080×1080（1:1）", 1080, 1080),
        ("小方图 500×500（1:1）", 500, 500),
        ("正文横图 1280×720（16:9）", 1280, 720),
        ("正文竖图 1080×1440（3:4）", 1080, 1440),
        ("贴图 900×383", 900, 383),
        ("贴图 383×383", 383, 383),
    ],
    note: "完整支持：方言富文本 + 草稿箱 API",
};

pub const XHS: PlatformSpec = PlatformSpec {
    id: "xhs",
    name_zh: "小红书",
    name_en: "Xiaohongshu",
    rich_text: false,
    image_note: true,
    api_publish: false,
    presets: &[
        ("封面 1080×1440（3:4）", 1080, 1440),
        ("正文图 1080×1440（3:4）", 1080, 1440),
        ("方图 1080×1080（1:1）", 1080, 1080),
    ],
    note: "图片笔记路径：海报/图组导出 + 文案复制；官方发布器贴文",
};

pub const ZHIHU: PlatformSpec = PlatformSpec {
    id: "zhihu",
    name_zh: "知乎",
    name_en: "Zhihu",
    rich_text: true,
    image_note: false,
    api_publish: false,
    presets: &[("封面 1920×1080（16:9）", 1920, 1080)],
    note: "Markdown/富文本友好（出口适配待接）",
};

pub const META: PlatformSpec = PlatformSpec {
    id: "meta",
    name_zh: "Meta (Facebook/Instagram)",
    name_en: "Meta (Facebook/Instagram)",
    rich_text: false,
    image_note: true,
    api_publish: false,
    presets: &[
        ("Instagram 方图 1080×1080", 1080, 1080),
        ("Instagram 竖图 1080×1350（4:5）", 1080, 1350),
        ("Story 1080×1920（9:16）", 1080, 1920),
    ],
    note: "需 Graph API 资质（v2 出口适配）",
};

pub const X: PlatformSpec = PlatformSpec {
    id: "x",
    name_zh: "X (Twitter)",
    name_en: "X (Twitter)",
    rich_text: false,
    image_note: false,
    api_publish: false,
    presets: &[
        ("横图 1600×900（16:9）", 1600, 900),
        ("方图 1080×1080（1:1）", 1080, 1080),
    ],
    note: "API v2 发文，280 字摘要策略（v2 出口适配）",
};

pub const LINKEDIN: PlatformSpec = PlatformSpec {
    id: "linkedin",
    name_zh: "LinkedIn",
    name_en: "LinkedIn",
    rich_text: false,
    image_note: false,
    api_publish: false,
    presets: &[
        ("横图 1200×627", 1200, 627),
        ("方图 1080×1080（1:1）", 1080, 1080),
    ],
    note: "文章 API 成熟但富文本有限（v2 出口适配）",
};

/// All platforms in switcher order (WeChat first: the default target).
pub fn list_platforms() -> Vec<PlatformSpec> {
    vec![
        WECHAT.clone(),
        XHS.clone(),
        ZHIHU.clone(),
        META.clone(),
        X.clone(),
        LINKEDIN.clone(),
    ]
}

/// Look a platform up by id; unknown ids fall back to WeChat so callers can
/// treat platform as a hint rather than a hard switch.
pub fn get_platform(id: &str) -> PlatformSpec {
    list_platforms()
        .into_iter()
        .find(|p| p.id == id)
        .unwrap_or(WECHAT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_wechat_is_default() {
        let all = list_platforms();
        assert!(all.len() >= 5);
        let mut ids: Vec<_> = all.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        assert_eq!(get_platform("wechat").id, "wechat");
        assert_eq!(get_platform("nonexistent").id, "wechat");
        assert_eq!(get_platform("xhs").name_zh, "小红书");
    }

    #[test]
    fn xhs_is_image_note_without_rich_text() {
        assert!(XHS.image_note && !XHS.rich_text);
        assert!(WECHAT.rich_text && !WECHAT.image_note);
    }

    #[test]
    fn every_platform_has_at_least_one_preset() {
        for p in list_platforms() {
            assert!(!p.presets.is_empty(), "{} has no presets", p.id);
        }
    }
}
