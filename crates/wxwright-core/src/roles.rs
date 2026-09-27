//! Canonical theme role table (PRD 8): the single source of truth for which
//! `[block.<role>]` keys a theme may target and the renderer actually reads.
//!
//! Three consumers share this table:
//!   1. `render.rs` - every key listed here is consulted while rendering;
//!   2. the GUI's AI theme prompt - the advertised role list is generated
//!      from here instead of being hand-written;
//!   3. `tests/theme_roles_test.rs` - asserts every advertised key reaches
//!      the rendered HTML.
//!
//! Why this module exists: the AI prompt advertised `card_note` and friends
//! while `render_card` computed the role and then discarded it
//! (`let _ = role;`). AI-authored card styling was silently thrown away and
//! nothing failed - the theme validator only checks blocking rule violations.
//! A shared table plus a test makes that class of drift impossible.

/// Which keys the renderer consumes for a role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleShape {
    /// Only `[block.<id>]`, applied to the block container.
    Block,
    /// `[block.<id>]` on the container plus `[block.<id>_leaf]` on text runs.
    BlockWithLeaf,
    /// Container, `_leaf` (body text) and `_title` (the card's label chip).
    CardWithTitle,
}

pub struct RoleDef {
    /// Base id as written in a theme TOML.
    pub id: &'static str,
    pub label_zh: &'static str,
    pub label_en: &'static str,
    pub shape: RoleShape,
}

pub const ROLES: &[RoleDef] = &[
    RoleDef {
        id: "h1",
        label_zh: "一级标题",
        label_en: "Heading 1",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "h2",
        label_zh: "二级标题",
        label_en: "Heading 2",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "h3",
        label_zh: "三级标题",
        label_en: "Heading 3",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "h4",
        label_zh: "四级标题",
        label_en: "Heading 4",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "h5",
        label_zh: "五级标题",
        label_en: "Heading 5",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "paragraph",
        label_zh: "正文段落",
        label_en: "Body paragraph",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "quote",
        label_zh: "引用块",
        label_en: "Blockquote",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "code",
        label_zh: "代码块容器",
        label_en: "Code block",
        shape: RoleShape::Block,
    },
    RoleDef {
        id: "table",
        label_zh: "表格容器",
        label_en: "Table wrapper",
        shape: RoleShape::Block,
    },
    RoleDef {
        id: "table_head",
        label_zh: "表头单元格",
        label_en: "Table head cell",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "table_cell",
        label_zh: "表格正文单元格",
        label_en: "Table body cell",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "list_item",
        label_zh: "列表项",
        label_en: "List item",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "figure_caption",
        label_zh: "图片图注",
        label_en: "Figure caption",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "toc",
        label_zh: "目录容器",
        label_en: "Table of contents",
        shape: RoleShape::Block,
    },
    RoleDef {
        id: "toc_item",
        label_zh: "目录条目",
        label_en: "TOC entry",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "toc_title",
        label_zh: "目录标题",
        label_en: "TOC heading",
        shape: RoleShape::Block,
    },
    RoleDef {
        id: "rule",
        label_zh: "分割线",
        label_en: "Horizontal rule",
        shape: RoleShape::Block,
    },
    RoleDef {
        id: "formula",
        label_zh: "公式块",
        label_en: "Formula block",
        shape: RoleShape::BlockWithLeaf,
    },
    RoleDef {
        id: "card_note",
        label_zh: "提示卡",
        label_en: "Note card",
        shape: RoleShape::CardWithTitle,
    },
    RoleDef {
        id: "card_tip",
        label_zh: "技巧卡",
        label_en: "Tip card",
        shape: RoleShape::CardWithTitle,
    },
    RoleDef {
        id: "card_important",
        label_zh: "重要卡",
        label_en: "Important card",
        shape: RoleShape::CardWithTitle,
    },
    RoleDef {
        id: "card_warning",
        label_zh: "警告卡",
        label_en: "Warning card",
        shape: RoleShape::CardWithTitle,
    },
    RoleDef {
        id: "card_caution",
        label_zh: "严重警告卡",
        label_en: "Caution card",
        shape: RoleShape::CardWithTitle,
    },
    RoleDef {
        id: "card_comment",
        label_zh: "评论卡",
        label_en: "Comment card",
        shape: RoleShape::CardWithTitle,
    },
    RoleDef {
        id: "card_keypoint",
        label_zh: "重点卡",
        label_en: "Key-point card",
        shape: RoleShape::CardWithTitle,
    },
];

/// Every theme key this build actually consumes, with `_leaf` / `_title`
/// variants expanded in render order.
pub fn all_keys() -> Vec<String> {
    let mut out = Vec::new();
    for r in ROLES {
        out.push(r.id.to_string());
        if matches!(r.shape, RoleShape::BlockWithLeaf | RoleShape::CardWithTitle) {
            out.push(format!("{}_leaf", r.id));
        }
        if r.shape == RoleShape::CardWithTitle {
            out.push(format!("{}_title", r.id));
        }
    }
    out
}

/// True when a theme key is one the renderer will actually read. A key that
/// is not known can never have an effect, so `validate_generated_theme`
/// rejects it rather than letting the AI write a theme that silently no-ops.
pub fn is_known_key(key: &str) -> bool {
    all_keys().iter().any(|k| k == key)
}

/// The role list embedded in the AI theme prompt, generated from `ROLES` so
/// the prompt can never advertise a role the renderer does not consume.
pub fn prompt_role_list() -> String {
    ROLES.iter().map(|r| r.id).collect::<Vec<_>>().join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_ids_are_unique() {
        let mut ids: Vec<&str> = ROLES.iter().map(|r| r.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate role id in ROLES");
    }

    #[test]
    fn keys_expand_per_shape() {
        let keys = all_keys();
        assert!(keys.contains(&"paragraph_leaf".to_string()));
        assert!(keys.contains(&"h1_leaf".to_string()));
        assert!(keys.contains(&"code".to_string()));
        assert!(
            !keys.contains(&"code_leaf".to_string()),
            "the code container has no consumed _leaf key"
        );
        assert!(keys.contains(&"card_note".to_string()));
        assert!(keys.contains(&"card_note_leaf".to_string()));
        assert!(keys.contains(&"card_note_title".to_string()));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(is_known_key("card_note"));
        assert!(is_known_key("card_keypoint_title"));
        assert!(!is_known_key("card_hover"));
        assert!(!is_known_key("a"));
        assert!(!is_known_key("code_leaf"));
    }

    #[test]
    fn prompt_list_covers_every_role() {
        let list = prompt_role_list();
        for r in ROLES {
            assert!(list.contains(r.id), "prompt list is missing {}", r.id);
        }
    }
}
