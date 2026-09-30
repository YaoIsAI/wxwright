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
    /// Content-derived variants the renderer may pick for this role, in the
    /// order they are documented to AI authors. A variant `v` contributes the
    /// theme keys `<id>.v` (container) and - per shape - `<id>.v_leaf`.
    /// Empty for roles that render identically regardless of content.
    pub variants: &'static [&'static str],
}

/// Convenience for the many roles that have no variants.
macro_rules! role {
    ($id:expr, $zh:expr, $en:expr, $shape:expr) => {
        RoleDef {
            id: $id,
            label_zh: $zh,
            label_en: $en,
            shape: $shape,
            variants: &[],
        }
    };
}

pub const ROLES: &[RoleDef] = &[
    role!("h1", "一级标题", "Heading 1", RoleShape::BlockWithLeaf),
    role!("h2", "二级标题", "Heading 2", RoleShape::BlockWithLeaf),
    role!("h3", "三级标题", "Heading 3", RoleShape::BlockWithLeaf),
    role!("h4", "四级标题", "Heading 4", RoleShape::BlockWithLeaf),
    role!("h5", "五级标题", "Heading 5", RoleShape::BlockWithLeaf),
    role!("h6", "六级标题", "Heading 6", RoleShape::BlockWithLeaf),
    RoleDef {
        id: "paragraph",
        label_zh: "正文段落",
        label_en: "Body paragraph",
        shape: RoleShape::BlockWithLeaf,
        // `lead` is the article's opening paragraph (the lede slot).
        variants: &["lead"],
    },
    RoleDef {
        id: "quote",
        label_zh: "引用块",
        label_en: "Blockquote",
        shape: RoleShape::BlockWithLeaf,
        // `hero` is a one-sentence standalone quote - the pull-quote slot.
        variants: &["hero"],
    },
    role!("code", "代码块容器", "Code block", RoleShape::Block),
    RoleDef {
        id: "table",
        label_zh: "表格容器",
        label_en: "Table wrapper",
        shape: RoleShape::Block,
        // `dense` fires on five or more columns: wide tables tighten to fit a
        // phone, and a theme can style the dense form separately.
        variants: &["dense"],
    },
    RoleDef {
        id: "table_head",
        label_zh: "表头单元格",
        label_en: "Table head cell",
        shape: RoleShape::BlockWithLeaf,
        variants: &["dense"],
    },
    RoleDef {
        id: "table_cell",
        label_zh: "表格正文单元格",
        label_en: "Table body cell",
        shape: RoleShape::BlockWithLeaf,
        variants: &["dense"],
    },
    role!("list_item", "列表项", "List item", RoleShape::BlockWithLeaf),
    role!(
        "figure_caption",
        "图片图注",
        "Figure caption",
        RoleShape::BlockWithLeaf
    ),
    role!("toc", "目录容器", "Table of contents", RoleShape::Block),
    role!(
        "toc_item",
        "目录条目",
        "TOC entry",
        RoleShape::BlockWithLeaf
    ),
    role!("toc_heading", "目录标题", "TOC heading", RoleShape::Block),
    role!("rule", "分割线", "Horizontal rule", RoleShape::Block),
    role!(
        "formula",
        "公式块",
        "Formula block",
        RoleShape::BlockWithLeaf
    ),
    role!("card_note", "提示卡", "Note card", RoleShape::CardWithTitle),
    role!("card_tip", "技巧卡", "Tip card", RoleShape::CardWithTitle),
    role!(
        "card_important",
        "重要卡",
        "Important card",
        RoleShape::CardWithTitle
    ),
    role!(
        "card_warning",
        "警告卡",
        "Warning card",
        RoleShape::CardWithTitle
    ),
    role!(
        "card_caution",
        "严重警告卡",
        "Caution card",
        RoleShape::CardWithTitle
    ),
    role!(
        "card_comment",
        "评论卡",
        "Comment card",
        RoleShape::CardWithTitle
    ),
    role!(
        "card_keypoint",
        "重点卡",
        "Key-point card",
        RoleShape::CardWithTitle
    ),
];

/// Every theme key this build actually consumes, with `_leaf` / `_title`
/// variants expanded in render order, followed by each role's content
/// variants (`role.variant`, `role.variant_leaf`).
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
        for v in r.variants {
            out.push(format!("{}.{}", r.id, v));
            if matches!(r.shape, RoleShape::BlockWithLeaf | RoleShape::CardWithTitle) {
                out.push(format!("{}.{}_leaf", r.id, v));
            }
        }
    }
    out
}

/// The variant keys embedded in the AI theme prompt, generated from `ROLES`
/// with their meaning, so the prompt teaches exactly the variants the
/// renderer computes - no more, no fewer.
pub fn prompt_variant_list() -> String {
    ROLES
        .iter()
        .flat_map(|r| r.variants.iter().map(move |v| (r.id, *v, r.shape)))
        .map(|(id, v, shape)| match shape {
            RoleShape::BlockWithLeaf | RoleShape::CardWithTitle => {
                format!("{}.{}（含 {}_leaf）", id, v, id)
            }
            RoleShape::Block => format!("{}.{}", id, v),
        })
        .collect::<Vec<_>>()
        .join("、")
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

    /// Variant keys live in the same key space as base roles: dotted, leaf
    /// suffixes expand per shape, and undeclared variants do not exist.
    #[test]
    fn variant_keys_expand_and_are_known() {
        assert!(is_known_key("quote.hero"));
        assert!(is_known_key("quote.hero_leaf"));
        assert!(is_known_key("paragraph.lead"));
        assert!(is_known_key("paragraph.lead_leaf"));
        // A role without variants declares nothing dotted.
        assert!(!is_known_key("code.hero"));
        // A variant the table does not declare must not be accepted, or a
        // theme could carry a key the renderer never computes (the h6 lesson,
        // mirrored for variants).
        assert!(!is_known_key("quote.lead"));
        assert!(!is_known_key("paragraph.hero"));
        assert!(!is_known_key("quote.hero_title"));
    }

    /// The prompt teaches every declared variant, and only those.
    #[test]
    fn prompt_variant_list_matches_the_table() {
        let list = prompt_variant_list();
        for r in ROLES {
            for v in r.variants {
                assert!(list.contains(&format!("{}.{}", r.id, v)));
            }
        }
        assert_eq!(list.matches("quote.hero").count(), 1);
        assert!(!list.contains("code."));
    }

    /// The renderer builds heading roles dynamically (`format!("h{}", level)`
    /// for every level pulldown-cmark can produce, 1..=6). The table used to
    /// stop at h5, so `validate_generated_theme` rejected `[block.h6]` even
    /// though the renderer honoured it - a "declared" gap rather than a
    /// "consumed" one, which the forward-only test could not see.
    #[test]
    fn every_heading_level_the_renderer_can_emit_is_declared() {
        for level in 1..=6u8 {
            let id = format!("h{level}");
            assert!(is_known_key(&id), "{id} must be declared");
            assert!(
                is_known_key(&format!("{id}_leaf")),
                "{id}_leaf must be declared"
            );
        }
        // pulldown-cmark tops out at h6; there is no h7 to declare.
        assert!(!is_known_key("h7"));
    }

    /// `all_keys()` must stay in step with `ROLES`: the expanded count is
    /// derived from the shapes plus the variant tables, so adding a role
    /// without a shape (or a variant without a declaration) is caught here
    /// rather than discovered in a theme that silently does nothing.
    #[test]
    fn key_count_is_derived_from_the_shapes() {
        let base_keys: usize = ROLES
            .iter()
            .map(|r| match r.shape {
                RoleShape::Block => 1,
                RoleShape::BlockWithLeaf => 2,
                RoleShape::CardWithTitle => 3,
            })
            .sum();
        // Variants contribute `role.variant` (+ `role.variant_leaf` per shape).
        let variant_keys: usize = ROLES
            .iter()
            .map(|r| {
                r.variants.len()
                    * match r.shape {
                        RoleShape::Block => 1,
                        RoleShape::BlockWithLeaf | RoleShape::CardWithTitle => 2,
                    }
            })
            .sum();
        assert_eq!(
            all_keys().len(),
            base_keys + variant_keys,
            "all_keys() disagrees with the ROLES shape arithmetic"
        );
        // Pinned so a role change is a deliberate edit, not an accident.
        assert_eq!(ROLES.len(), 26, "role count changed - update the docs too");
        assert_eq!(
            base_keys, 54,
            "base key count changed - update the docs too"
        );
        assert_eq!(
            variant_keys, 9,
            "variant key count changed - update the docs too"
        );
        assert_eq!(all_keys().len(), 63);
    }

    /// Every `BlockWithLeaf` / `CardWithTitle` role must expand its suffix
    /// variants, and every `Block` role must not claim any - including the
    /// dotted variant keys.
    #[test]
    fn shapes_expand_consistently() {
        for r in ROLES {
            let has_leaf = is_known_key(&format!("{}_leaf", r.id));
            let has_title = is_known_key(&format!("{}_title", r.id));
            match r.shape {
                RoleShape::Block => {
                    assert!(!has_leaf, "{} is Block but declares _leaf", r.id);
                    assert!(!has_title, "{} is Block but declares _title", r.id);
                }
                RoleShape::BlockWithLeaf => {
                    assert!(has_leaf, "{} must declare _leaf", r.id);
                    assert!(!has_title, "{} must not declare _title", r.id);
                }
                RoleShape::CardWithTitle => {
                    assert!(has_leaf, "{} must declare _leaf", r.id);
                    assert!(has_title, "{} must declare _title", r.id);
                }
            }
            for v in r.variants {
                assert!(
                    is_known_key(&format!("{}.{}", r.id, v)),
                    "{}.{} must be declared",
                    r.id,
                    v
                );
                let variant_leaf = is_known_key(&format!("{}.{}_leaf", r.id, v));
                match r.shape {
                    RoleShape::Block => {
                        assert!(
                            !variant_leaf,
                            "{}.{} is Block but declares a _leaf",
                            r.id, v
                        );
                    }
                    _ => {
                        assert!(variant_leaf, "{}.{} must declare a _leaf", r.id, v);
                    }
                }
            }
        }
    }

    #[test]
    fn prompt_list_covers_every_role() {
        let list = prompt_role_list();
        for r in ROLES {
            assert!(list.contains(r.id), "prompt list is missing {}", r.id);
        }
    }
}
