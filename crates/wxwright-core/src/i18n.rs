//! Minimal i18n for human-facing engine output (PRD 3.7).
//! Machine-facing surfaces (JSON schema, rule IDs, exit codes) are English
//! constants and never localized. Missing keys fall back to English.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    ZhCn,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::ZhCn => "zh-CN",
        }
    }

    pub fn from_code(s: &str) -> Option<Lang> {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "en_us" | "c" | "posix" => Some(Lang::En),
            "zh-cn" | "zh_cn" | "zh-hans" | "zh" => Some(Lang::ZhCn),
            _ => None,
        }
    }
}

static LANG: AtomicU8 = AtomicU8::new(0); // 0 = En, 1 = ZhCn

pub fn set_lang(l: Lang) {
    LANG.store(
        match l {
            Lang::En => 0,
            Lang::ZhCn => 1,
        },
        Ordering::Relaxed,
    );
}

pub fn get_lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::ZhCn,
        _ => Lang::En,
    }
}

type Entry = (&'static str, &'static str, &'static str); // (key, en, zh-CN)

const CATALOG: &[Entry] = &[
    ("card.note", "Note", "笔记"),
    ("card.tip", "Tip", "提示"),
    ("card.important", "Important", "重要"),
    ("card.warning", "Warning", "注意"),
    ("card.caution", "Caution", "警告"),
    ("card.comment", "Comments", "留言"),
    ("card.keypoint", "Key Points", "划重点"),
    ("links.title", "References", "参考链接"),
    ("toc.title", "CONTENTS", "目录"),
    ("image.missing", "Image unavailable", "图片无法加载"),
    ("figure.caption", "Caption", "图注"),
];

pub fn t(key: &str) -> String {
    let entry = CATALOG.iter().find(|(k, _, _)| *k == key);
    match entry {
        Some((_, _, zh)) if get_lang() == Lang::ZhCn => zh.to_string(),
        Some((_, en, _)) => en.to_string(),
        None => key.to_string(),
    }
}

/// Translate with `{name}` parameter substitution.
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(key);
    for (k, v) in args {
        s = s.replace(&format!("{{{}}}", k), v);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_and_switch() {
        set_lang(Lang::En);
        assert_eq!(t("card.note"), "Note");
        assert_eq!(t("nonexistent.key"), "nonexistent.key");
        set_lang(Lang::ZhCn);
        assert_eq!(t("card.note"), "笔记");
        set_lang(Lang::En);
    }

    #[test]
    fn lang_codes() {
        assert_eq!(Lang::from_code("zh-CN"), Some(Lang::ZhCn));
        assert_eq!(Lang::from_code("fr"), None);
    }
}
