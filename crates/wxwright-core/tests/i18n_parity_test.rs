//! i18n key-parity and machine-surface stability tests (PRD 9-6, 3.7-B).

use wxwright_core::i18n::{set_lang, t, Lang};
use wxwright_core::rules::RULES;

const KEYS: &[&str] = &[
    "card.note",
    "card.tip",
    "card.important",
    "card.warning",
    "card.caution",
    "card.comment",
    "card.keypoint",
    "links.title",
    "toc.title",
    "image.missing",
    "figure.caption",
];

#[test]
fn every_key_resolves_in_both_langs() {
    for key in KEYS {
        set_lang(Lang::En);
        let en = t(key);
        assert_ne!(en, *key, "missing en translation for {}", key);
        set_lang(Lang::ZhCn);
        let zh = t(key);
        assert_ne!(zh, *key, "missing zh-CN translation for {}", key);
        assert_ne!(en, zh, "en and zh identical for {}", key);
    }
    set_lang(Lang::En);
}

#[test]
fn rule_descriptions_bilingual_and_ids_stable() {
    for r in RULES {
        assert!(r.id.starts_with("R-") || r.id == "HYGIENE");
        assert!(!r.en.is_empty());
        assert!(!r.zh.is_empty());
    }
}

#[test]
fn lang_code_parsing() {
    assert_eq!(Lang::from_code("en"), Some(Lang::En));
    assert_eq!(Lang::from_code("zh-CN"), Some(Lang::ZhCn));
    assert_eq!(Lang::from_code("zh_CN"), Some(Lang::ZhCn));
    assert_eq!(Lang::from_code("fr"), None);
}
