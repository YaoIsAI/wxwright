//! Official WeChat editor spec rule metadata (PRD 5.3).
//! Rule IDs are stable English constants (machine-facing contract);
//! descriptions are localized at report time (PRD 3.7-B).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// R-1/R-2/R-3: hard violation, blocks the pipeline.
    Block,
    /// R-4 (Dark Mode tuning): warning only.
    Warn,
}

#[derive(Debug, Clone, Copy)]
pub struct RuleInfo {
    pub id: &'static str,
    pub severity: Severity,
    pub auto_fixable: bool,
    pub en: &'static str,
    pub zh: &'static str,
}

macro_rules! rule {
    ($id:literal, $sev:ident, $fix:literal, $en:literal, $zh:literal) => {
        RuleInfo {
            id: $id,
            severity: Severity::$sev,
            auto_fixable: $fix == 1,
            en: $en,
            zh: $zh,
        }
    };
}

pub const RULES: &[RuleInfo] = &[
    rule!(
        "R-1.1",
        Block,
        1,
        "img opacity:0 + SVG background trick breaks image re-editing in the MP editor",
        "img opacity:0 + SVG 背景图叠放技巧会导致后台无法二次编辑图片"
    ),
    rule!(
        "R-1.2",
        Block,
        1,
        "caret-color: transparent is forbidden",
        "禁止 caret-color 完全透明"
    ),
    rule!(
        "R-1.3",
        Block,
        1,
        "line-height must not be smaller than the font size (stacked text on mobile)",
        "line-height 不得小于字号，否则移动端叠字"
    ),
    rule!(
        "R-1.4",
        Block,
        1,
        "fixed pixel width causes mis-centering/overflow; use percentage widths",
        "禁固定像素宽度（居中不一致/溢出/比例失真），应使用百分比"
    ),
    rule!(
        "R-1.4w",
        Warn,
        0,
        "img should carry data-w (original pixel width) to avoid false load-timeout reports",
        "img 建议携带 data-w（原始像素宽），避免加载超时误报"
    ),
    rule!(
        "R-1.5.1",
        Block,
        1,
        "text containers must not use height:0 (invisible body text on mobile)",
        "含文字容器禁用 height:0（移动端正文不可见）"
    ),
    rule!(
        "R-1.5.2",
        Block,
        1,
        "fixed small height must not clip text content (scroll containers exempt)",
        "禁固定小高度裁剪文字内容（滚动容器豁免）"
    ),
    rule!(
        "R-1.6",
        Block,
        1,
        "text-align: start/end breaks iOS; use left/right",
        "text-align 禁用 start/end（iOS 兼容差），应使用 left/right"
    ),
    rule!(
        "R-1.7",
        Block,
        1,
        "SVG animate begin must not listen to touchstart only (broken on PC)",
        "SVG animate begin 不得只写 touchstart（PC 端失效）"
    ),
    rule!(
        "R-1.8",
        Block,
        1,
        "plain text must not use <pre> (white-space:pre truncates on mobile)",
        "普通正文禁用 <pre>（white-space:pre 移动端截断）"
    ),
    rule!(
        "R-2.1",
        Block,
        0,
        "nested chains of same-tag same-style wrappers must not exceed 10 levels",
        "同标签+同内联样式+单子节点的嵌套链不得超过 10 层"
    ),
    rule!(
        "R-2.2",
        Block,
        1,
        "block-level elements are forbidden inside span[leaf]",
        "span[leaf] 内禁止块级元素"
    ),
    rule!(
        "R-2.3",
        Block,
        1,
        "section[nodeleaf] may only contain official components or img",
        "section[nodeleaf] 只能包含官方组件或 img"
    ),
    rule!(
        "R-3.1",
        Block,
        1,
        "font-family must never be set (breaks editor/mobile consistency)",
        "禁止设置任何 font-family（破坏编辑器/移动端一致性）"
    ),
    rule!(
        "R-4.1.1",
        Warn,
        0,
        "extremely low or high text/background contrast triggers Dark Mode algorithm rewrites",
        "文字/背景对比度过低或过高会触发 Dark Mode 算法改写"
    ),
    rule!(
        "R-4.1.2",
        Warn,
        1,
        "gradient background under text is converted to a solid color in Dark Mode",
        "文字下方渐变背景在 Dark Mode 下会被转为纯色"
    ),
    rule!(
        "R-4.2",
        Warn,
        0,
        "absolute positioning breaks visual/structural order under the Dark Mode traversal",
        "绝对定位会破坏 Dark Mode 深度优先遍历的视觉/结构顺序"
    ),
    rule!(
        "R-4.3",
        Warn,
        0,
        "images should not carry plain text; transparent images need contrast care",
        "图片不承载纯文本；透明图需注意与 #191919 底色的对比"
    ),
    rule!(
        "R-4.4",
        Warn,
        0,
        "SVG content is not converted in Dark Mode",
        "SVG 内容在 Dark Mode 下不做转换"
    ),
    rule!(
        "HYGIENE",
        Block,
        1,
        "payload hygiene: no script/style/iframe/form, no class/id, no event handlers",
        "载荷卫生：不含 script/style/iframe/form、class/id 与事件属性"
    ),
];

pub fn rule_info(id: &str) -> Option<&'static RuleInfo> {
    RULES.iter().find(|r| r.id == id)
}

impl RuleInfo {
    /// Localized description (PRD 3.7-A-③: rule id stays English).
    pub fn localized_desc(&self) -> String {
        match crate::i18n::get_lang() {
            crate::i18n::Lang::ZhCn => self.zh.to_string(),
            crate::i18n::Lang::En => self.en.to_string(),
        }
    }

    pub fn is_block(&self) -> bool {
        self.severity == Severity::Block
    }
}
