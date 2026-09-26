
/* wxwright GUI: library + editor + preview + AI assistant + agent panel
   + poster studio + device frames + writing pet (墨仔). */
"use strict";

const $ = (id) => document.getElementById(id);
const invoke = window.__TAURI__ ? window.__TAURI__.core.invoke : null;
const listen = window.__TAURI__ ? window.__TAURI__.event.listen : null;

/* ------------------------------------------------------------------ i18n */
const I18N = {
  "zh-CN": {
    library: "文章库", theme: "主题", copy: "复制富文本", ai: "AI 助手",
    ready: "就绪", converting: "转换中...", not_saved: "未保存", saved: "已保存",
    save: "保存", empty_library: "还没有文章\n点击右上角 + 新建",
    search_ph: "搜索文章...",
    chars: "字符", words: "词数", images: "图片",
    settings: "设置", about: "关于", ai_providers: "AI Providers（OpenAI 兼容协议）",
    ai_hint: "支持 OpenAI / DeepSeek / 通义千问 / Kimi / 智谱等云端服务，以及本地 Ollama（http://localhost:11434）与 LM Studio（http://localhost:1234）。API Key 存入系统钥匙串，设置文件只存引用。",
    f_name: "名称", f_model: "模型", f_baseurl: "Base URL", f_key: "API Key",
    f_logo: "自定义图标（可选，data URI / 上传）",
    f_key_hint: "留空表示保留原 Key", test: "测试连接", cancel: "取消", ok: "确定",
    save_provider: "保存 Provider", active_badge: "使用中",
    ai_assistant: "AI 助手", clear: "清空", send_placeholder: "向 AI 描述你的需求，Enter 发送，Shift+Enter 换行",
    q_polish: "润色当前文章", q_continue: "续写", q_title: "起 5 个标题", q_outline: "帮我列提纲", q_poster: "生成头图文案",
    ai_insert: "插入编辑器", ai_replace: "替换文章", ai_copy: "复制",
    agent_title: "Agent 接入", agent_mcp: "MCP 一键接入",
    agent_mcp_hint: "点击对应客户端，wxwright 的 MCP 配置（wxwright mcp serve）会自动写入；写入后重启客户端即可看到 wxwright 工具。",
    agent_card: "Agent 接手卡", agent_card_hint: "把整段卡片复制进任意 AI Agent 的系统提示，Agent 即刻接手；或让它直接调用 CLI / MCP。",
    copy_card: "复制整段卡片", preview_card: "预览卡片", hide_card: "收起卡片",
    agent_cli: "CLI 速查（点击复制）",
    c_copy: "全链路：转换-图片-校验-剪贴板", c_convert: "转换为公众号 HTML",
    c_validate: "规范校验（退出码 0/1/2）", c_serve: "手动启动 MCP server（stdio）",
    c_card: "输出 Agent 接手卡", c_doctor: "环境体检",
    copied_ok: "富文本已复制。打开公众号编辑器，Ctrl+V 粘贴。",
    copied_plain: "剪贴板已写入（纯文本模式）。",
    copy_blocked_rules: "存在阻断级规范问题，未复制：",
    copy_blocked_images: "以下图片无法通过剪贴板粘贴（需 mmbiz 或 https 直链）：",
    copy_failed: "失败：", exported_ok: "已导出：",
    rule_ok: "规范通过", rule_chip: "规范",
    dark_hint: "深色预览为模拟效果，实际以公众号 Dark Mode 算法为准",
    "violations_none": "未发现规范问题。",
    ft_title: "没有 API Key？",
    ft_body: "作者的另一个项目 Free Tokens 持续收录免费模型额度——去逛逛，一起实现 Token 自由。",
    svg_desc_ph: "描述想要的互动效果，例如：点击后头像放大并显示一句祝福语",
    saved_ok: (x) => `已保存：${x}`, saved_new_ok: (x) => `已自动保存为新文章：${x}`, deleted_ok: "已删除", renamed_ok: "已重命名", duplicated_ok: "已创建副本", imported_ok: "已导入",
    copied_text: "已复制到剪贴板", mcp_installed: (p) => `已写入：${p}`,
    provider_saved: "Provider 已保存", connected: "连接成功：",
    first_run_hint: "提示：右上角机器人图标可一键把 wxwright 接入 Claude / Cursor 等 AI Agent",
    demo_mode: "浏览器演示模式（无本地后端）",
    theme_ai_title: "AI 生成主题", theme_ai_hint: "描述你想要的主题风格，AI 会生成全新配色与排版，并自动通过公众号官方规范校验（font-family 禁用、对比度约束）。生成结果保存为用户主题，CLI 也能使用。",
    generate: "生成主题", theme_generating: "生成中，约需 10-30 秒...",
    theme_done: (id) => `主题「${id}」已生成并应用`, theme_need_ai: "请先在设置中配置 AI Provider",
    poster_title: "海报工坊（HTML 生成图片）", preset: "尺寸",
    poster_desc_ph: "描述海报内容与风格，点「AI 生成」", ai_generate: "AI 生成",
    poster_hint: "本地光栅化（SVG foreignObject），完全离线。海报 HTML 必须自包含：禁止外部图片/字体/脚本，图片只能内嵌 data URI。适合做公众号头图、金句卡、正文场景图。",
    reset_tpl: "重置模板", save_img: "保存图片...", insert_article: "导出并插入文章",
    poster_inserted: "图片已插入文章", poster_saved: "图片已保存", poster_need_ai: "请先在设置中配置 AI Provider",
    poster_generating: "AI 生成 HTML 中...", poster_rasterizing: "正在导出 PNG...",
    rename: "重命名", duplicate: "创建副本", prompt_rename: "重命名文章",
    img_imported: (n) => `已导入 ${n} 张图片`,
    nav_providers: "AI Providers", nav_wx: "公众号 API",
    wx_title: "公众号 API 绑定",
    wx_hint: "绑定后可使用「推送草稿」直接把文章写入公众号草稿箱。AppID / AppSecret 存入系统钥匙串，仅保存在本机。",
    wx_appid: "AppID", wx_secret: "AppSecret",
    wx_bind: "绑定", wx_unbind: "解除绑定",
    wx_bind_ok: "已绑定公众号", wx_unbind_ok: "已解除绑定",
    wx_demo_only: "浏览器演示模式下不可用",
    wx_secret_ph: "仅保存到本机钥匙串",
    wx_status_fail: "读取绑定状态失败",
    validate: "校验",
    platform: "平台", custom_size: "自定义...",
    copy_rich: "复制富文本", copy_caption: "复制文案", copy_md: "复制 Markdown",
    copied_caption: "文案已复制，去小红书 App 粘贴", copied_md: "Markdown 已复制",
    platform_title: "目标平台：切换后预览、规则与导出联动",
    theme_ai_btn: "AI 生成主题", export_title: "导出 HTML",
    comfy_title: "AI 绘图（ComfyUI 本地）", svgkit_title: "SVG 互动组件库",
    assets_title: "素材库", ai_assistant_btn: "AI 助手", agent_title: "Agent 接入",
    import_md: "导入 .md 文件", new_article: "新建文章",
    validate_title: "校验公众号规范",
    attach_title: "上传附件（文档 PDF/DOCX/HTML/TXT/MD + 图片走视觉模型）",
    model_select_title: "切换模型", send_title: "发送", dark_toggle: "深色 / 浅色",
    pet_title: "墨仔（点击摸摸，双击换形态）",
    vendor_preset: "预设厂商（选择后自动填充）", vendor_preset_ph: "— 选择厂商自动填充 —",
    logo_ph: "留空使用内置品牌标", upload_logo: "上传图标", clear: "清除",
    comfy_section_title: "ComfyUI（本地 AI 绘图）",
    comfy_section_hint: "检测到本机 ComfyUI 后，「AI 绘图」按钮即可调用本地 Stable Diffusion 文生图 / 图生图，产物直接进入素材库。",
    comfy_url_label: "ComfyUI 地址", comfy_model_label: "Checkpoint 模型",
    comfy_launch_label: "启动程序路径（用于一键启动）", comfy_start: "一键启动",
    about_line1: "wxwright 0.9.0 · by AI瑶（微信公众号：码聋）",
    about_line2: "写作小宠物「墨仔」住在左下角状态栏，记得去摸摸它。",
    chip_jp: "日系手账", chip_jp_desc: "日系手账风，奶油色底，橙棕强调色，圆角便签卡片，温柔文艺",
    chip_cyber: "赛博科技", chip_cyber_desc: "赛博科技感，深色代码面板，霓虹青蓝强调色，等宽律动",
    chip_mag: "复古杂志", chip_mag_desc: "复古杂志编辑风，暖纸色，朱红强调色，居中大标题与双细线",
    chip_academy: "墨绿学院", chip_academy_desc: "墨绿学院风，米白纸面，墨绿与金色点缀，庄重书卷气",
    theme_desc_ph: "例如：奶茶铺子配色，奶咖色底、焦糖强调色，圆角卡片，元气手写感",
    tpl_cover: "头图模板", tpl_quote: "金句贴图卡", tpl_pic: "图文贴图卡",
    open_folder: "打开素材文件夹", imgsrc_cloud: "云端图像 API",
    image_model_label: "图像模型（保存后即可用云端生成）", save_model: "保存模型",
    comfy_launch_ph: "ComfyUI 启动程序路径（如 D:\ComfyUI\run_nvidia_gpu.bat）",
    comfy_prompt_label: "提示词（正向）", comfy_negative_label: "反向提示词（留空用默认）",
    comfy_i2i_row: "图生图源图（从素材库选择）", comfy_pick: "从素材库选",
    mode_t2i: "文生图", mode_i2i: "图生图", generate_btn: "生成",
    qr_title: "公众号 · 码聋", qr_line1: "微信扫码关注 码聋", qr_line2: "AI瑶 的写作与技术专栏",
    fit_title: "尺寸工坊（适配微信标准）", fit_target: "目标尺寸",
    fit_mode: "适配方式", fit_cover: "裁切填满（居中裁剪）", fit_contain: "完整置入（补白）",
    fit_scale: "导出倍数", fit_2x: "2x（更清晰）", fit_bg: "补白背景",
    bg_white: "白", bg_black: "黑", bg_lightgray: "浅灰", bg_brand: "品牌浅蓝",
    fit_hint: "导出后自动进入素材库",
    csize_cover: "1080 × 460（头图 2.35:1）", csize_square: "1080 × 1080（次图 1:1）",
    csize_small: "500 × 500（小方图）", csize_wide: "1280 × 720（横图 16:9）", csize_tall: "1080 × 1440（竖图 3:4）",
    svgkit_hint: "组件在公众号里由读者「点击 / 触摸」触发动画（自动满足官方 R-1.7 规则）。选组件、改文字与颜色、看右侧实时预览，然后插入文章；插入后引擎会再做一次合规校验。也可以直接让 AI 按描述生成全新组件，或上传自定义图片参与互动。",
    imgset: "导出图组", imgset_done: (n) => `图组已导出 ${n} 张到素材库`,
    job_stopped: "已停止",
  },
  en: {
    library: "Library", theme: "Theme", copy: "Copy rich text", ai: "AI",
    ready: "Ready", converting: "Converting...", not_saved: "unsaved", saved: "saved",
    save: "Save", empty_library: "No articles yet.\nClick + to create.",
    search_ph: "Search articles...",
    chars: "chars", words: "words", images: "images",
    settings: "Settings", about: "About", ai_providers: "AI Providers (OpenAI-compatible)",
    ai_hint: "Works with OpenAI / DeepSeek / Qwen / Kimi / Zhipu and local Ollama (http://localhost:11434) or LM Studio (http://localhost:1234). API keys go to the OS keychain.",
    f_name: "Name", f_model: "Model", f_baseurl: "Base URL", f_key: "API Key",
    f_logo: "Custom icon (optional, data URI / upload)",
    f_key_hint: "leave empty to keep the current key", test: "Test", cancel: "Cancel", ok: "OK",
    save_provider: "Save provider", active_badge: "active",
    ai_assistant: "AI Assistant", clear: "Clear", send_placeholder: "Describe what you need. Enter to send, Shift+Enter for newline",
    q_polish: "Polish article", q_continue: "Continue writing", q_title: "5 title ideas", q_outline: "Draft an outline", q_poster: "Cover copy",
    ai_insert: "Insert", ai_replace: "Replace article", ai_copy: "Copy",
    agent_title: "Agent integration", agent_mcp: "One-click MCP setup",
    agent_mcp_hint: "Click a client to write the wxwright MCP config (wxwright mcp serve). Restart the client afterwards.",
    agent_card: "Agent card", agent_card_hint: "Paste this card into any AI agent's system prompt, or let it call the CLI / MCP directly.",
    copy_card: "Copy card", preview_card: "Preview card", hide_card: "Hide card",
    agent_cli: "CLI cheat sheet (click to copy)",
    c_copy: "Full chain: convert-images-validate-clipboard", c_convert: "Convert to MP HTML",
    c_validate: "Compliance check (exit 0/1/2)", c_serve: "Start MCP server manually (stdio)",
    c_card: "Print the agent card", c_doctor: "Environment check",
    copied_ok: "Rich text copied. Open the MP editor and press Ctrl+V.",
    copied_plain: "Clipboard written (plain-text mode).",
    copy_blocked_rules: "Blocking violations, not copied:",
    copy_blocked_images: "These images cannot survive clipboard paste (mmbiz or https required):",
    copy_failed: "Failed: ", exported_ok: "Exported: ",
    rule_ok: "Compliant", rule_chip: "Rules",
    dark_hint: "Dark preview is an approximation; the MP Dark Mode algorithm decides",
    "violations_none": "No violations found.",
    ft_title: "No API key?",
    ft_body: "The author's other project Free Tokens curates free model credits - token freedom for everyone.",
    svg_desc_ph: "Describe the interaction, e.g. avatar zooms in with a blessing on tap",
    saved_ok: (x) => `Saved: ${x}`, saved_new_ok: (x) => `Auto-saved as new article: ${x}`, deleted_ok: "Deleted", renamed_ok: "Renamed", duplicated_ok: "Duplicated", imported_ok: "Imported",
    copied_text: "Copied to clipboard", mcp_installed: (p) => `Written: ${p}`,
    provider_saved: "Provider saved", connected: "Connected: ",
    first_run_hint: "Tip: the robot icon top-right connects wxwright to Claude / Cursor in one click",
    demo_mode: "Browser demo mode (no local backend)",
    theme_ai_title: "AI theme generator", theme_ai_hint: "Describe the style; the AI generates a brand-new palette and typography, automatically validated against the official MP rules. Saved as a user theme (usable from the CLI too).",
    generate: "Generate", theme_generating: "Generating, 10-30s...",
    theme_done: (id) => `Theme "${id}" generated and applied`, theme_need_ai: "Configure an AI provider in Settings first",
    poster_title: "Poster Studio (HTML to PNG)", preset: "Size",
    poster_desc_ph: "Describe the poster, then click AI Generate", ai_generate: "AI Generate",
    poster_hint: "Local rasterization (SVG foreignObject), fully offline. Poster HTML must be self-contained: no external images/fonts/scripts; data-URI images only.",
    reset_tpl: "Reset template", save_img: "Save image...", insert_article: "Export & insert",
    poster_inserted: "Image inserted into the article", poster_saved: "Image saved", poster_need_ai: "Configure an AI provider in Settings first",
    poster_generating: "Generating HTML...", poster_rasterizing: "Exporting PNG...",
    rename: "Rename", duplicate: "Duplicate", prompt_rename: "Rename article",
    img_imported: (n) => `${n} image(s) imported`,
    nav_providers: "AI Providers", nav_wx: "MP API",
    wx_title: "Official Account API binding",
    wx_hint: "After binding, \"Push draft\" writes articles straight to your MP draft box. AppID / AppSecret are stored in the system keychain, on this machine only.",
    wx_appid: "AppID", wx_secret: "AppSecret",
    wx_bind: "Bind", wx_unbind: "Unbind",
    wx_bind_ok: "Official account bound", wx_unbind_ok: "Unbound",
    wx_demo_only: "Unavailable in browser demo mode",
    wx_secret_ph: "Keychain on this machine only",
    wx_status_fail: "Failed to read binding status",
    validate: "Check",
    platform: "Platform", custom_size: "Custom...",
    copy_rich: "Copy rich text", copy_caption: "Copy caption", copy_md: "Copy Markdown",
    copied_caption: "Caption copied - paste it into the note editor", copied_md: "Markdown copied",
    platform_title: "Target platform: preview, rules and export follow it",
    theme_ai_btn: "AI theme generator", export_title: "Export HTML",
    comfy_title: "AI drawing (local ComfyUI)", svgkit_title: "SVG interactive kit",
    assets_title: "Assets", ai_assistant_btn: "AI assistant", agent_title: "Agent integration",
    import_md: "Import .md", new_article: "New article",
    validate_title: "Check MP compliance",
    attach_title: "Attach documents (PDF/DOCX/HTML/TXT/MD; images use vision)",
    model_select_title: "Switch model", send_title: "Send", dark_toggle: "Dark / Light",
    pet_title: "Mozai (click to pet, double-click to morph)",
    vendor_preset: "Vendor preset (auto-fills)", vendor_preset_ph: "- pick a vendor to auto-fill -",
    logo_ph: "empty = built-in brand mark", upload_logo: "Upload icon", clear: "Clear",
    comfy_section_title: "ComfyUI (local AI drawing)",
    comfy_section_hint: "With a local ComfyUI detected, the AI drawing button drives local Stable Diffusion txt2img / img2img; results land in the asset library.",
    comfy_url_label: "ComfyUI URL", comfy_model_label: "Checkpoint model",
    comfy_launch_label: "Launcher path (for one-click start)", comfy_start: "Start",
    about_line1: "wxwright 0.9.0 - by AI Yao (MP: MaLong)",
    about_line2: "Mozai the writing pet lives in the bottom-left status bar - go pet it.",
    chip_jp: "Journal", chip_jp_desc: "Japanese journal style: cream paper, caramel accent, rounded note cards",
    chip_cyber: "Cyber", chip_cyber_desc: "Cyber tech style: dark code panels, neon cyan-blue accent, mono rhythm",
    chip_mag: "Magazine", chip_mag_desc: "Vintage magazine style: warm paper, vermilion accent, centered headlines",
    chip_academy: "Academy", chip_academy_desc: "Academy style: ivory paper, deep green and gold, classic serif mood",
    theme_desc_ph: "e.g. milk-tea shop palette, latte background, caramel accent, rounded cards",
    tpl_cover: "Cover template", tpl_quote: "Quote card", tpl_pic: "Picture card",
    open_folder: "Open assets folder", imgsrc_cloud: "Cloud image API",
    image_model_label: "Image model (save to enable cloud generation)", save_model: "Save model",
    comfy_launch_ph: "ComfyUI launcher path (e.g. D:\ComfyUI\run_nvidia_gpu.bat)",
    comfy_prompt_label: "Prompt (positive)", comfy_negative_label: "Negative prompt (blank = default)",
    comfy_i2i_row: "img2img source (pick from assets)", comfy_pick: "Pick from assets",
    mode_t2i: "txt2img", mode_i2i: "img2img", generate_btn: "Generate",
    qr_title: "Official Account - MaLong", qr_line1: "Scan to follow MaLong", qr_line2: "AI Yao's writing & tech column",
    fit_title: "Size studio (MP sizes)", fit_target: "Target size",
    fit_mode: "Fit mode", fit_cover: "Cover (center crop)", fit_contain: "Contain (pad)",
    fit_scale: "Export scale", fit_2x: "2x (sharper)", fit_bg: "Pad background",
    bg_white: "White", bg_black: "Black", bg_lightgray: "Light gray", bg_brand: "Brand blue",
    fit_hint: "Exports land in the asset library",
    csize_cover: "1080 × 460 (cover 2.35:1)", csize_square: "1080 × 1080 (square 1:1)",
    csize_small: "500 × 500 (small square)", csize_wide: "1280 × 720 (wide 16:9)", csize_tall: "1080 × 1440 (tall 3:4)",
    svgkit_hint: "Components animate on reader tap inside MP (auto-compliant with rule R-1.7). Pick one, tweak text/colours, watch the live preview, then insert; the engine re-validates on insert. You can also let AI generate a brand-new component or upload a custom image.",
    imgset: "Export image set", imgset_done: (n) => `${n} image(s) exported to the asset library`,
    job_stopped: "Stopped",
  },
};
let lang = "zh-CN";

/* ----------------------------------------------------------------- state */
let currentTheme = "minimal";
let darkPreview = false;
let convertTimer = null;
let aiLanding = false; // one-shot: pulse the preview's last block after AI insert/replace
const jobBusy = {}; // in-flight guard per AI panel kind
let lastResult = null;
let currentArticleId = null;
let dirty = false;
let aiBusy = false;
let aiHistory = [];
let libraryFilter = "";

/* ------------------------------------------------------------------ util */
function t(key, ...args) {
  const v = I18N[lang][key];
  // missing keys degrade to the key name itself (iron law 4: never undefined)
  if (v === undefined) return key;
  return typeof v === "function" ? v(...args) : v;
}
function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
function toast(message, kind = "info", listHtml = "") {
  const host = $("toast-host");
  const el = document.createElement("div");
  el.className = `toast ${kind}`;
  el.innerHTML = `<div><div>${message}</div>${listHtml}</div>`;
  host.appendChild(el);
  setTimeout(() => el.remove(), kind === "err" ? 8000 : 3600);
}
async function copyPlain(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (e) {
    if (invoke) {
      try { await invoke("copy_text_plain", { text }); return true; } catch (e2) {}
    }
    return false;
  }
}
function applyI18n() {
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const k = el.getAttribute("data-i18n");
    if (I18N[lang][k] && typeof I18N[lang][k] === "string") el.textContent = I18N[lang][k];
  });
  document.querySelectorAll("[data-i18n-placeholder]").forEach((el) => {
    const k = el.getAttribute("data-i18n-placeholder");
    if (I18N[lang][k]) el.placeholder = I18N[lang][k];
  });
  document.querySelectorAll("[data-i18n-title]").forEach((el) => {
    const k = el.getAttribute("data-i18n-title");
    if (I18N[lang][k]) el.title = I18N[lang][k];
  });
  document.querySelectorAll("[data-desc-i18n]").forEach((el) => {
    const k = el.getAttribute("data-desc-i18n");
    if (I18N[lang][k]) el.dataset.desc = I18N[lang][k];
  });
  if (typeof refreshCopyButton === "function") refreshCopyButton();
  $("ai-input").placeholder = t("send_placeholder");
  $("library-search").placeholder = t("search_ph");
  const langBtn = $("btn-lang");
  if (langBtn) langBtn.textContent = lang === "zh-CN" ? "EN" : "中文";
  const ftLink = $("freetokens-link");
  if (ftLink && !ftLink.dataset.bound) {
    ftLink.dataset.bound = "1";
    ftLink.addEventListener("click", () => {
      if (window.__TAURI__ && window.__TAURI__.opener) {
        window.__TAURI__.opener.openUrl("https://free-tokens.org");
      } else {
        window.open("https://free-tokens.org", "_blank");
      }
    });
  }
  localStorage.setItem("wxwright-lang", lang);
}

/* -------------------------------------------------------------- preview */
function wrapPreviewHtml(dialect) {
  // one-shot landing pulse on the newest last block after AI insert/replace
  const pulse = aiLanding
    ? `<style>
  @keyframes wxland{0%{box-shadow:inset 4px 0 0 0 var(--accent,#2F6CEA);background:rgba(47,108,234,.10);transform:translateY(6px);opacity:.2}
    60%{box-shadow:inset 4px 0 0 0 var(--accent,#2F6CEA);background:rgba(47,108,234,.08)}
    100%{box-shadow:none;background:transparent;transform:none;opacity:1}}
  body>*:last-child{animation:wxland 1.5s ease both;border-radius:6px;}
</style>`
    : "";
  aiLanding = false;
  return `<!doctype html><html><head><meta charset="utf-8">
<style>
  html,body{margin:0;padding:0;background:#fff;}
  body{padding:16px 14px;word-break:break-word;}
  section{max-width:100%;}
  /* phone-realistic: no visible scrollbars */
  ::-webkit-scrollbar{width:0;height:0;display:none;}
  html{scrollbar-width:none;}
</style>${pulse}</head><body>${dialect}</body></html>`;
}
function updateStats(stats, violations) {
  $("stat-chars").textContent = `${t("chars")} ${stats.chars}`;
  $("stat-words").textContent = `${t("words")} ${stats.words}`;
  $("stat-images").textContent = `${t("images")} ${stats.images}`;
  updateRuleChip(violations || []);
}
function updateRuleChip(violations) {
  const chip = $("rule-chip");
  chip.hidden = false;
  const blocks = violations.filter((v) => v.severity === "block").length;
  const warns = violations.length - blocks;
  if (violations.length === 0) {
    chip.className = "rule-chip ok";
    chip.innerHTML = `<svg class="icon icon-sm"><use href="#i-check"/></svg><span>${t("rule_ok")}</span>`;
  } else {
    chip.className = "rule-chip" + (blocks > 0 ? " blocking" : "");
    chip.innerHTML = `<span>${t("rule_chip")}</span><b class="rc-block">${blocks}</b><span>${lang === "zh-CN" ? "阻断" : "block"}</span>·<b class="rc-warn">${warns}</b><span>${lang === "zh-CN" ? "提示" : "warn"}</span>`;
  }
}
function renderViolationsPanel(violations) {
  const panel = $("violations-panel");
  if (!violations || violations.length === 0) {
    panel.innerHTML = `<div class="violation-row"><span class="v-desc">${t("violations_none")}</span></div>`;
    return;
  }
  panel.innerHTML = violations
    .map(
      (v) => `<div class="violation-row">
        <span class="v-badge ${v.severity}">${v.severity === "block" ? "BLOCK" : "WARN"}</span>
        <span class="v-rule">${v.rule_id}</span>
        <span class="v-desc">${escapeHtml(v.message)}</span>
        <span class="v-node">${escapeHtml(v.node)}</span>
      </div>`
    )
    .join("");
}
async function convertNow() {
  const md = $("editor").value;
  if (!invoke) {
    // Browser demo: channel shells still render so every platform shape is
    // reviewable without the desktop shell (caption is derived from the editor).
    if (currentPlatform !== "wechat") {
      $("preview").srcdoc = currentPlatform === "zhihu"
        ? zhihuArticleShell(demoPlainHtmlFromMd(md))
        : renderChannelShell(currentPlatform, {
            mode: "note",
            platform: currentPlatform,
            title: loadedTitle || autoTitle(md),
            caption: demoCaptionFromMd(md),
            images: [],
          });
    } else {
      try {
        $("preview").srcdoc = await fetch("demo-preview.html").then((r) => r.text());
        updateRuleChip([]);
      } catch (e) {}
    }
    // demo-side stats and status reset (desktop gets both from convert_preview)
    const demoImgs = (md.match(/!\[[^\]]*\]\([^)]*\)|<img\s/g) || []).length;
    const demoWords = md.trim() ? md.trim().split(/\s+/).length : 0;
    $("stat-chars").textContent = `${t("chars")} ${md.length}`;
    $("stat-words").textContent = `${t("words")} ${demoWords}`;
    $("stat-images").textContent = `${t("images")} ${demoImgs}`;
    $("status-text").textContent = t("ready");
    return;
  }
  try {
    const res = await invoke("convert_preview", { markdown: md, themeId: currentTheme });
    lastResult = res;
    // channel preview: same article, the target platform's own layout
    if (currentPlatform !== "wechat") {
      try {
        const model = await invoke("platform_preview", {
          platform: currentPlatform,
          title: loadedTitle || autoTitle(md),
          markdown: md,
          themeId: currentTheme,
        });
        $("preview").srcdoc = model.mode === "note"
          ? renderChannelShell(model.platform, model)
          : zhihuArticleShell(model.html);
      } catch (e) {
        $("preview").srcdoc = wrapPreviewHtml(res.html); // fall back to dialect
      }
    } else {
      $("preview").srcdoc = wrapPreviewHtml(res.html);
    }
    updateStats(res.stats, res.violations);
    if (currentPlatform !== "wechat") {
      // platform rule table replaces the dialect verdict for non-WeChat targets
      try {
        const pv = await invoke("platform_validate", {
          platform: currentPlatform,
          title: loadedTitle || autoTitle(md),
          markdown: md,
          images: res.stats.images || 0,
        });
        updateRuleChip(pv.violations || []);
        if (!$("violations-panel").hidden) renderViolationsPanel(pv.violations || []);
      } catch (e) { /* keep dialect verdict on failure */ }
    }
    if (!$("violations-panel").hidden) renderViolationsPanel(res.violations);
    $("status-text").textContent = t("ready");
  } catch (e) {
    $("status-text").textContent = String(e);
  }
}
function scheduleConvert() {
  $("status-text").textContent = t("converting");
  clearTimeout(convertTimer);
  convertTimer = setTimeout(convertNow, 220);
}

async function loadThemesText() {
  if (!invoke) return;
  try {
    const themes = await invoke("list_themes");
    $("theme-select").innerHTML = themes
      .map((x) => `<option value="${x.id}">${escapeHtml(lang === "zh-CN" ? x.name_zh || x.name : x.name)}</option>`)
      .join("");
    $("theme-select").value = currentTheme;
  } catch (e) {}
}

/* -------------------------------------------------------------- library */
function autoTitle(md) {
  const m = md.match(/^#\s+(.+)$/m);
  return m ? m[1].trim() : (lang === "zh-CN" ? "未命名文章" : "Untitled");
}
async function refreshLibrary() {
  const list = $("article-list");
  if (!invoke) {
    const demoTitle = lang === "zh-CN" ? "wxwright 一分钟上手（演示）" : "wxwright quickstart (demo)";
    if (libraryFilter && !demoTitle.toLowerCase().includes(libraryFilter.toLowerCase())) {
      list.innerHTML = `<div class="sidebar-empty">${t("empty_library")}</div>`;
      return;
    }
    list.innerHTML = `<div class="article-item" data-id="demo"><div class="a-main"><div class="a-title">${demoTitle}</div><div class="a-date">2026-09-25</div></div></div>`;
    return;
  }
  let articles = await invoke("list_articles");
  if (libraryFilter) {
    const f = libraryFilter.toLowerCase();
    articles = articles.filter((a) => a.title.toLowerCase().includes(f));
  }
  if (articles.length === 0) {
    list.innerHTML = `<div class="sidebar-empty">${t("empty_library")}</div>`;
    return;
  }
  list.innerHTML = articles
    .map(
      (a) => `<div class="article-item${a.id === currentArticleId ? " active" : ""}" data-id="${escapeHtml(a.id)}" title="${escapeHtml(a.title)}">
        <div class="a-main">
          <div class="a-title">${escapeHtml(a.title)}</div>
          <div class="a-date">${escapeHtml((a.updated || a.created).replace("T", " "))}</div>
        </div>
        <button class="a-act" title="${t("rename")}" data-rename="${escapeHtml(a.id)}"><svg class="icon icon-sm"><use href="#i-edit"/></svg></button>
        <button class="a-act" title="${t("duplicate")}" data-dup="${escapeHtml(a.id)}"><svg class="icon icon-sm"><use href="#i-copy"/></svg></button>
        <button class="a-act danger" title="delete" data-del="${escapeHtml(a.id)}"><svg class="icon icon-sm"><use href="#i-trash"/></svg></button>
      </div>`
    )
    .join("");
  list.querySelectorAll(".article-item").forEach((el) => {
    el.addEventListener("click", (e) => {
      if (e.target.closest(".a-act")) return;
      openArticle(el.dataset.id);
    });
  });
  list.querySelectorAll("[data-del]").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      e.stopPropagation();
      await invoke("delete_article", { id: btn.dataset.del });
      if (currentArticleId === btn.dataset.del) currentArticleId = null;
      toast(t("deleted_ok"), "ok");
      refreshLibrary();
    });
  });
  list.querySelectorAll("[data-rename]").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      e.stopPropagation();
      const art = await invoke("read_article", { id: btn.dataset.rename });
      const newTitle = await uiPrompt(t("prompt_rename"), art.meta.title);
      if (newTitle && newTitle.trim() && newTitle !== art.meta.title) {
        await invoke("save_article", { id: art.meta.id, title: newTitle.trim(), theme: art.meta.theme, platform: art.meta.platform, markdown: art.markdown });
        toast(t("renamed_ok"), "ok");
        refreshLibrary();
      }
    });
  });
  list.querySelectorAll("[data-dup]").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      e.stopPropagation();
      const art = await invoke("read_article", { id: btn.dataset.dup });
      await invoke("save_article", { id: null, title: art.meta.title + (lang === "zh-CN" ? "（副本）" : " (copy)"), theme: art.meta.theme, platform: art.meta.platform, markdown: art.markdown });
      toast(t("duplicated_ok"), "ok");
      refreshLibrary();
    });
  });
}
let loadedTitle = null; // title of the article currently open (library truth)
async function persistCurrent(silent = false) {
  if (!invoke) return;
  const md = $("editor").value;
  if (md.trim().length === 0) return;
  // Title rules: a library article keeps its stored title (renames survive
  // edits); a brand-new unsaved doc takes its first heading.
  const title = currentArticleId && loadedTitle ? loadedTitle : autoTitle(md);
  const isNew = !currentArticleId;
  const meta = await invoke("save_article", {
    id: currentArticleId,
    title,
    theme: currentTheme,
    platform: currentPlatform,
    markdown: md,
  });
  currentArticleId = meta.id;
  loadedTitle = meta.title;
  dirty = false;
  $("stat-saved").textContent = t("saved");
  if (!silent) {
    toast(isNew ? t("saved_new_ok", meta.title) : t("saved_ok", meta.title), "ok");
  } else if (isNew) {
    toast(t("saved_new_ok", meta.title));
  }
  window.Mozai && Mozai.celebrate();
  refreshLibrary();
}
async function openArticle(id) {
  if (!invoke) return;
  if (dirty) {
    // An opened library article updates itself; a never-saved draft is
    // stored as a NEW article - tell the user that's what happened.
    if (currentArticleId === null) {
      const md = $("editor").value;
      if (md.trim().length > 0) {
        const meta = await invoke("save_article", { id: null, title: autoTitle(md), theme: currentTheme, platform: currentPlatform, markdown: md });
        toast(t("saved_new_ok", meta.title));
      }
    } else {
      await persistCurrent(true);
    }
  }
  const art = await invoke("read_article", { id });
  currentArticleId = art.meta.id;
  loadedTitle = art.meta.title;
  $("editor").value = art.markdown;
  dirty = false;
  $("stat-saved").textContent = t("saved");
  if (art.meta.theme) {
    currentTheme = art.meta.theme;
    $("theme-select").value = currentTheme;
    localStorage.setItem("wxwright-theme", currentTheme);
  }
  if (art.meta.platform && art.meta.platform !== currentPlatform) {
    applyPlatform(art.meta.platform); // silent: platform is a per-article property
  }
  refreshLibrary();
  convertNow();
}
async function newArticle() {
  if (invoke && $("editor").value.trim().length > 0 && dirty) await persistCurrent(true);
  currentArticleId = null;
  loadedTitle = null;
  $("editor").value = "# " + (lang === "zh-CN" ? "新文章" : "New article") + "\n\n";
  $("stat-saved").textContent = t("not_saved");
  dirty = false;
  refreshLibrary();
  $("editor").focus();
  convertNow();
}
async function importMdFiles() {
  if (!invoke) { toast(t("demo_mode"), "err"); return; }
  try {
    const { open } = window.__TAURI__.dialog;
    const path = await open({
      multiple: false,
      filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
    });
    if (!path) return;
    const content = await invoke("read_text_file", { path });
    await invoke("save_article", { id: null, title: autoTitle(content), theme: currentTheme, platform: currentPlatform, markdown: content });
    toast(t("imported_ok"), "ok");
    refreshLibrary();
  } catch (e) {
    toast(String(e), "err");
  }
}

/* --------------------------------------------------------- prompt modal */
function uiPrompt(title, value = "") {
  return new Promise((resolve) => {
    $("prompt-title").textContent = title;
    $("prompt-input").value = value;
    openModal("modal-prompt");
    $("prompt-input").focus();
    $("prompt-input").select();
    const done = (v) => {
      closeModal("modal-prompt");
      $("prompt-ok").removeEventListener("click", okH);
      $("prompt-cancel").removeEventListener("click", cancelH);
      $("prompt-input").removeEventListener("keydown", keyH);
      resolve(v);
    };
    const okH = () => done($("prompt-input").value.trim() || null);
    const cancelH = () => done(null);
    const keyH = (e) => {
      if (e.key === "Enter") okH();
      if (e.key === "Escape") done(null);
    };
    $("prompt-ok").addEventListener("click", okH);
    $("prompt-cancel").addEventListener("click", cancelH);
    $("prompt-input").addEventListener("keydown", keyH);
  });
}

/* -------------------------------------------------------------- actions */
async function doCopy() {
  if (!invoke) { toast(t("demo_mode"), "err"); return; }
  // Non-WeChat platforms: the copy artifact is the platform's own export
  // (caption text for image-note platforms, raw Markdown for Zhihu).
  if (currentPlatform !== "wechat") {
    const btn = $("btn-copy");
    btn.disabled = true;
    try {
      const text = await invoke("platform_export_text", {
        platform: currentPlatform,
        title: loadedTitle || autoTitle($("editor").value),
        markdown: $("editor").value,
      });
      await invoke("copy_text_plain", { text });
      toast(currentPlatform === "zhihu" ? t("copied_md") : t("copied_caption"), "ok");
      window.Mozai && Mozai.celebrate();
    } catch (e) {
      toast(String(e), "err");
    } finally {
      btn.disabled = false;
    }
    return;
  }
  const btn = $("btn-copy");
  btn.disabled = true;
  try {
    const res = await invoke("copy_rich", { markdown: $("editor").value, themeId: currentTheme });
    if (res.copied) {
      toast(res.html_flavor ? t("copied_ok") : t("copied_plain"), "ok");
      window.Mozai && Mozai.celebrate();
    } else if (res.reason === "blocking violations" && res.blocking.length > 0) {
      const list = `<ul class="toast-list">${res.blocking.slice(0, 5).map((v) => `<li>${v.rule_id}: ${escapeHtml(v.message)}</li>`).join("")}</ul>`;
      toast(t("copy_blocked_rules"), "err", list);
    } else if (res.reason === "paste-hostile images" && res.paste_hostile_images.length > 0) {
      const list = `<ul class="toast-list">${res.paste_hostile_images.slice(0, 5).map((s) => `<li>${escapeHtml(s)}</li>`).join("")}</ul>`;
      toast(t("copy_blocked_images"), "err", list);
    } else {
      toast(t("copy_failed") + (res.reason || "unknown"), "err");
    }
  } catch (e) {
    toast(t("copy_failed") + String(e), "err");
  } finally {
    btn.disabled = false;
  }
}
async function doExport() {
  if (!invoke) { toast(t("demo_mode"), "err"); return; }
  try {
    const { save } = window.__TAURI__.dialog;
    const path = await save({
      title: "Export HTML",
      defaultPath: "wxwright-export.html",
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
    if (!path) return;
    const saved = await invoke("export_html", { markdown: $("editor").value, themeId: currentTheme, path });
    toast(t("exported_ok") + saved, "ok");
  } catch (e) {
    toast(t("copy_failed") + String(e), "err");
  }
}
async function doValidate() {
  await convertNow();
  $("violations-panel").hidden = false;
  renderViolationsPanel(lastResult ? lastResult.violations : []);
}

/* ------------------------------------------------------------- AI chat */
function renderMd(src) {
  const blocks = [];
  let s = escapeHtml(src).replace(/```(\w*)\n?([\s\S]*?)```/g, (m, l, c) => {
    blocks.push(`<pre class="md-pre"><code>${c.replace(/\n+$/, "")}</code></pre>`);
    return "\u0000" + (blocks.length - 1) + "\u0000";
  });
  s = s.replace(/`([^`\n]+)`/g, "<code class='md-icode'>$1</code>");
  s = s.replace(/^###?\s+(.+)$/gm, "<div class='md-h'>$1</div>");
  s = s.replace(/\*\*([^*\n]+)\*\*/g, "<strong>$1</strong>");
  s = s.replace(/(^|[^*])\*([^*\n]+)\*/g, "$1<em>$2</em>");
  s = s.replace(/^[-*]\s+(.+)$/gm, "<div class='md-li'>&bull;&nbsp;$1</div>");
  s = s.replace(/^\d+\.\s+(.+)$/gm, (m) => `<div class='md-li'>${escapeHtml(m.match(/^\d+/)[0])}.&nbsp;${m.replace(/^\d+\.\s+/, "")}</div>`);
  s = s.replace(/\n{2,}/g, "<div class='md-gap'></div>");
  s = s.replace(/\n/g, "<br/>");
  s = s.replace(/\u0000(\d+)\u0000/g, (m, i) => blocks[+i]);
  return s;
}
function aiProviderChip(settings) {
  const s = settings || { providers: [], active: null };
  const p = s.providers.find((x) => x.id === s.active);
  $("ai-provider-chip").textContent = p ? `${p.name} / ${p.model}` : (lang === "zh-CN" ? "未配置" : "not configured");
  return p;
}
async function refreshProviderChip() {
  if (!invoke) {
    $("ai-provider-chip").textContent = lang === "zh-CN" ? "未配置" : "not configured";
    return;
  }
  aiProviderChip(await invoke("ai_settings"));
}
let aiStopped = false;
let aiStartedAt = 0;
let aiModelName = "";
let msgPinned = true;

function msgScrollPinned() {
  const host = $("ai-messages");
  return host.scrollHeight - host.scrollTop - host.clientHeight < 72;
}
function msgScrollBottom(force) {
  const host = $("ai-messages");
  if (force || msgPinned) host.scrollTop = host.scrollHeight;
}
function updateJumpChip() {
  const chip = $("ai-jump");
  if (!chip) return;
  chip.hidden = msgPinned || !$("ai-messages").children.length;
}

function appendAiMessage(role, content, streaming = false) {
  const host = $("ai-messages");
  const el = document.createElement("div");
  el.className = `ai-msg ${role}`;
  const meta = role === "assistant" && aiModelName ? `${aiModelName}` : (role === "user" ? (lang === "zh-CN" ? "你" : "You") : "AI");
  el.innerHTML = `<div class="role">${meta}</div>
    <div class="bubble"><span class="content"></span>${streaming ? '<span class="ai-typing"><i></i><i></i><i></i></span>' : ""}</div>`;
  if (content) el.querySelector(".content").textContent = content;
  host.appendChild(el);
  msgScrollBottom(true);
  return el;
}

function aiThinkingTimer(el, intervalHandleRef) {
  const role = el.querySelector(".role");
  intervalHandleRef.t = setInterval(() => {
    const secs = ((Date.now() - aiStartedAt) / 1000).toFixed(0);
    role.textContent = `${aiModelName || "AI"} · ${lang === "zh-CN" ? "思考中" : "thinking"} ${secs}s`;
  }, 500);
}

function finalizeAiMessage(el, content, info = {}) {
  if (!el || !el.isConnected) return;
  el.querySelector(".cursor")?.remove();
  el.querySelector(".ai-typing")?.remove();
  const dur = aiStartedAt ? ((Date.now() - aiStartedAt) / 1000).toFixed(1) : null;
  const usage = info.usage;
  const tokens = usage && usage.completion_tokens ? `${usage.completion_tokens} tok` : null;
  const role = el.querySelector(".role");
  role.textContent = [aiModelName || "AI", info.stopped ? (lang === "zh-CN" ? "已停止" : "stopped") : null, dur ? dur + "s" : null, tokens]
    .filter(Boolean).join(" · ");
  const contentEl = el.querySelector(".content");
  contentEl.classList.add("md");
  contentEl.innerHTML = renderMd(content);
  const actions = document.createElement("div");
  actions.className = "msg-actions";
  const actionsSpec = [
    [t("ai_insert"), () => insertAtCursor(content)],
    [t("ai_replace"), () => {
      const ed = $("editor");
      ed.value = content;
      dirty = true;
      $("stat-saved").textContent = t("not_saved");
      ed.focus();
      ed.setSelectionRange(0, Math.min(400, content.length));
      flashEditor();
      aiLanding = true;
      convertNow();
    }],
    [t("ai_copy"), async () => { await copyPlain(content); toast(t("copied_text"), "ok"); }],
  ];
  for (const [label, fn] of actionsSpec) {
    const b = document.createElement("button");
    b.className = "chip";
    b.textContent = label;
    b.addEventListener("click", fn);
    actions.appendChild(b);
  }
  el.appendChild(actions);
  msgScrollBottom();
}

function insertAtCursor(text) {
  const ed = $("editor");
  const start = ed.selectionStart ?? ed.value.length;
  const before = ed.value.slice(0, start);
  const after = ed.value.slice(ed.selectionEnd ?? start);
  ed.value = before + text + after;
  dirty = true;
  $("stat-saved").textContent = t("not_saved");
  // visual guidance: select the inserted range + glow the editor
  ed.focus();
  ed.setSelectionRange(start, start + text.length);
  flashEditor();
  if (window.Mozai) Mozai.typing();
  aiLanding = true;
  scheduleConvert();
}
/* brief glow on the editor pane so the user sees where AI content landed */
function flashEditor() {
  const ed = $("editor");
  ed.classList.add("land-flash");
  clearTimeout(flashEditor._t);
  flashEditor._t = setTimeout(() => ed.classList.remove("land-flash"), 1200);
}
function buildQuickPrompt(kind) {
  const md = $("editor").value;
  const ctx = md.trim().length > 0 ? `

当前文章内容：

${md.slice(0, 6000)}` : "";
  const zh = lang === "zh-CN";
  switch (kind) {
    case "polish": return (zh ? "请润色下面这篇公众号文章，保持 Markdown 格式与结构，直接输出润色后的全文。" : "Polish this article, keep Markdown format, output the full text.") + ctx;
    case "continue": return (zh ? "请为下面的公众号文章续写 2-3 个段落，保持风格一致，直接输出续写内容。" : "Continue this article by 2-3 paragraphs in the same style.") + ctx;
    case "title": return (zh ? "为下面的公众号文章起 5 个吸引人的标题，每行一个，不要编号。" : "Propose 5 catchy titles for this article, one per line.") + ctx;
    case "outline": return (zh ? "我想写一篇公众号文章，请给出一份结构清晰的文章提纲（Markdown 列表）。主题：请结合我的描述。" : "Draft a clear article outline (Markdown list). Topic: see my description.") + ctx;
    case "poster": return (zh ? "为公众号头图写一句主标题文案与一句副标题文案（共两行，直接输出，不要解释），主题结合当前文章。" : "Write one headline and one subline for an MP cover image, two lines only.") + ctx;
    default: return kind;
  }
}
const AI_SYSTEM = () => lang === "zh-CN"
  ? "你是一位微信公众号写作助手。始终输出标准 Markdown（GFM）。风格自然、信息密度高、适合移动端阅读。可以使用引用提示卡语法（> [!NOTE] / [!KEYPOINT] 等）与表格。需要展示数据时优先使用图表围栏：```chart\\n{\"kind\":\"bar|line|pie\",\"title\":\"标题\",\"labels\":[\"标签\"…],\"values\":[数值…],\"unit\":\"单位(可选)\"}\\n```，labels 与 values 数量必须一致，pie 的 values 表示占比。"
  : "You are a WeChat Official Account writing assistant. Always output standard Markdown (GFM). Natural style, high information density, mobile-friendly. You may use blockquote alert syntax (> [!NOTE] / [!KEYPOINT]) and tables. When presenting data, prefer chart fences: ```chart\\n{\"kind\":\"bar|line|pie\",\"title\":\"...\",\"labels\":[...],\"values\":[...],\"unit\":\"(optional)\"}\\n``` — labels and values must match in length; pie values are proportions.";


async function aiSend(text) {
  if (!invoke) { demoStream(text); return; }
  if (aiBusy) return;
  const baseText = text.trim();
  if (!baseText) return;
  aiBusy = true;
  aiStopped = false;
  aiStartedAt = Date.now();
  if (window.Mozai) Mozai.setBusy(true);
  // attachments: text docs fold into the prompt; images become vision parts
  let content = baseText;
  const visionParts = [];
  const textAtts = [];
  for (const a of pendingAttachments) {
    if (a.kind === "image") {
      visionParts.push({ type: "image_url", image_url: { url: a.dataUri } });
    } else {
      textAtts.push(`${lang === "zh-CN" ? "【附件" : "[Attachment"} ${a.name}】\n${a.content}`);
    }
  }
  if (textAtts.length > 0) {
    content = textAtts.join("\n\n") + "\n\n" + content;
  }
  pendingAttachments = [];
  renderAttachRow();
  try {
    const st = await invoke("ai_settings");
    aiModelName = st.providers.find((p) => p.id === st.active)?.model || "AI";
  } catch (e) { aiModelName = "AI"; }
  const sendBtn = $("btn-ai-send");
  sendBtn.innerHTML = '<svg class="icon"><use href="#i-stop"/></svg>';
  sendBtn.classList.add("stopping");
  sendBtn.title = lang === "zh-CN" ? "停止生成" : "Stop";
  let userContent;
  let userEcho = content;
  if (visionParts.length > 0) {
    userContent = [{ type: "text", text: content }, ...visionParts.map((v) => ({ type: "image_url", image_url: v.image_url }))];
    appendAiMessage("user", content + (lang === "zh-CN" ? `（含 ${visionParts.length} 张图片）` : ` (with ${visionParts.length} image(s))`));
  } else {
    appendAiMessage("user", content);
  }
  aiHistory.push({ role: "user", content });
  const el = appendAiMessage("assistant", "", true);
  const contentEl = el.querySelector(".content");
  const timerRef = {};
  aiThinkingTimer(el, timerRef);
  let acc = "";
  let gotFirst = false;
  // incremental append: a dedicated text node (no full-text rewrites)
  let textNode = document.createTextNode("");
  const onChunk = (ev) => {
    if (!gotFirst) {
      gotFirst = true;
      clearInterval(timerRef.t);
      el.querySelector(".ai-typing")?.remove();
      contentEl.appendChild(textNode);
    }
    textNode.appendData(ev.payload);
    acc += ev.payload;
    requestAnimationFrame(() => msgScrollBottom());
  };
  const onDone = (ev) => {
    cleanup();
    const stopped = !!(ev && ev.payload && ev.payload.stopped);
    const usage = ev && ev.payload ? ev.payload.usage : null;
    if (!acc) acc = stopped ? (lang === "zh-CN" ? "（已停止）" : "(stopped)") : "";
    finalizeAiMessage(el, acc, { stopped, usage });
    if (!stopped) {
      aiHistory.push({ role: "assistant", content: acc });
      if (aiHistory.length > 24) aiHistory = aiHistory.slice(-24);
    }
  };
  const onError = (ev) => {
    cleanup();
    el.classList.add("error");
    contentEl.textContent = acc || (ev.payload || "error");
    el.querySelector(".ai-typing")?.remove();
    el.querySelector(".cursor")?.remove();
  };
  const un1 = listen("ai-chunk", onChunk);
  const un2 = listen("ai-done", onDone);
  const un3 = listen("ai-error", onError);
  function cleanup() {
    aiBusy = false;
    if (window.Mozai) Mozai.setBusy(false);
    clearInterval(timerRef.t);
    sendBtn.innerHTML = '<svg class="icon"><use href="#i-send"/></svg>';
    sendBtn.classList.remove("stopping");
    sendBtn.title = lang === "zh-CN" ? "发送" : "Send";
    un1.then((f) => f());
    un2.then((f) => f());
    un3.then((f) => f());
    updateJumpChip();
  }
  try {
    await invoke("ai_chat", {
      messages: [{ role: "system", content: AI_SYSTEM() }, ...aiHistory],
      temperature: 0.7,
    });
  } catch (e) {
    // errors surface via ai-error; keep partial text visible
  }
}

/* demo harness: canned markdown streamed locally so the whole UX is
   verifiable without a backend */
async function demoStream(text) {
  if (aiBusy) { toast(lang === "zh-CN" ? "正在生成中，请稍候" : "Still generating, please wait"); return; }
  aiBusy = true;
  aiStopped = false;
  aiStartedAt = Date.now();
  aiModelName = "demo";
  const sendBtn = $("btn-ai-send");
  sendBtn.innerHTML = '<svg class="icon"><use href="#i-stop"/></svg>';
  sendBtn.classList.add("stopping");
  appendAiMessage("user", text);
  const el = appendAiMessage("assistant", "", true);
  const contentEl = el.querySelector(".content");
  let acc = "";
  let gotFirst = false;
  let textNode = document.createTextNode("");
  const demoText = [
    "# 墨仔写作建议\n",
    "把这段改成**三段式**：开头一句话给结论，中间给证据，结尾给行动。\n\n",
    "- 先写烂，再改好\n- 删掉一半，文章就活了\n\n",
    "```rust\nfn main() {\n    println!(\"hello 码聋\");\n}\n```\n\n",
    "需要我把这套结构直接**替换进文章**吗？",
  ].join("");
  let i = 0;
  const CH = 24; // chars per tick
  const timer = setInterval(() => {
    if (aiStopped) { clearInterval(timer); finish(true); return; }
    if (i >= demoText.length) { clearInterval(timer); finish(false); return; }
    const next = demoText.slice(i, i + CH);
    i += CH;
    if (!gotFirst) {
      gotFirst = true;
      el.querySelector(".ai-typing")?.remove();
      contentEl.appendChild(textNode);
    }
    textNode.appendData(next);
    acc += next;
    requestAnimationFrame(() => msgScrollBottom());
  }, 90);
  function finish(stopped) {
    aiBusy = false;
    sendBtn.innerHTML = '<svg class="icon"><use href="#i-send"/></svg>';
    sendBtn.classList.remove("stopping");
    if (!el.isConnected) return; // drawer cleared while streaming
    if (!acc) acc = stopped ? (lang === "zh-CN" ? "（已停止）" : "(stopped)") : "";
    finalizeAiMessage(el, acc, { stopped });
    if (!stopped) {
      aiHistory.push({ role: "assistant", content: acc });
      if (aiHistory.length > 24) aiHistory = aiHistory.slice(-24);
    }
  }
}

/* ---------------------------------------------------------- AI settings *//* ---------------------------------------------------------- AI settings */
function openModal(id) { $(id).hidden = false; }
function closeModal(id) { $(id).hidden = true; }
async function renderProviderList() {
  const host = $("provider-list");
  if (!invoke) {
    host.innerHTML = `<div class="provider-row"><div class="p-main"><div class="p-name">DeepSeek（演示）</div><div class="p-meta">https://api.deepseek.com · deepseek-chat</div></div></div>`;
    return;
  }
  const s = await invoke("ai_settings");
  aiProviderChip(s);
  host.innerHTML =
    s.providers.length === 0
      ? `<div class="sidebar-empty">${lang === "zh-CN" ? "尚未添加 Provider" : "No provider yet"}</div>`
      : s.providers
          .map(
            (p) => `<div class="provider-row${p.id === s.active ? " active" : ""}" data-id="${escapeHtml(p.id)}">
              <span class="brand-tile p-logo">${brandTileHTML(p, 22)}</span>
              <div class="p-main">
                <div class="p-name">${escapeHtml(p.name)}</div>
                <div class="p-meta">${escapeHtml(p.base_url)} · ${escapeHtml(p.model)} · ${escapeHtml(p.key_hint)}</div>
              </div>
              ${p.id === s.active ? `<span class="p-badge">${t("active_badge")}</span>` : ""}
              <button type="button" class="btn btn-ghost btn-icon p-edit" title="edit" data-edit="${escapeHtml(p.id)}"><svg class="icon icon-sm"><use href="#i-gear"/></svg></button>
              <button type="button" class="p-del" title="delete" data-del="${escapeHtml(p.id)}"><svg class="icon icon-sm"><use href="#i-trash"/></svg></button>
            </div>`
          )
          .join("");
  host.querySelectorAll(".provider-row").forEach((row) => {
    row.addEventListener("click", async (e) => {
      if (e.target.closest(".p-del") || e.target.closest(".p-edit")) return;
      const s2 = await invoke("ai_set_active", { id: row.dataset.id });
      renderProviderList();
      aiProviderChip(s2);
    });
  });
  host.querySelectorAll("[data-edit]").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      e.stopPropagation();
      const s2 = await invoke("ai_settings");
      const p = s2.providers.find((x) => x.id === btn.dataset.edit);
      if (!p) return;
      $("pf-id").value = p.id;
      $("pf-name").value = p.name;
      $("pf-model").value = p.model;
      $("pf-base").value = p.base_url;
      $("pf-brand").value = p.brand || "";
      $("pf-key").value = "";
      $("pf-logo").value = p.logo || "";
      $("pf-key").placeholder = t("f_key_hint");
    });
  });
  host.querySelectorAll("[data-del]").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      e.stopPropagation();
      const s2 = await invoke("ai_delete_provider", { id: btn.dataset.del });
      renderProviderList();
      aiProviderChip(s2);
    });
  });
}
async function saveProviderFromForm() {
  const provider = {
    id: $("pf-id").value || "",
    name: $("pf-name").value.trim(),
    base_url: $("pf-base").value.trim(),
    model: $("pf-model").value.trim(),
  };
  const key = $("pf-key").value.trim();
  provider.brand = $("pf-brand").value.trim() || null;
  provider.logo = $("pf-logo").value.trim() || null;
  const s = key
    ? await invoke("ai_save_provider_with_key", { provider, apiKey: key })
    : await invoke("ai_save_provider", { provider });
  ["pf-id", "pf-name", "pf-model", "pf-base", "pf-key", "pf-logo"].forEach((id) => ($(id).value = ""));
  renderProviderList();
  aiProviderChip(s);
  return s;
}
async function hasProvider() {
  if (!invoke) return false;
  const s = await invoke("ai_settings");
  return !!(s.active && s.providers.find((p) => p.id === s.active));
}

/* ---------------------------------------------------------- agent panel */
async function loadAgentCard() {
  const pre = $("agent-card-pre");
  if (pre.dataset.loaded === "1") return;
  if (invoke) {
    pre.textContent = await invoke("agent_card_markdown");
  } else {
    try {
      pre.textContent = await fetch("demo-agent-card.md").then((r) => r.text());
    } catch (e) {
      pre.textContent = "demo";
    }
  }
  pre.dataset.loaded = "1";
}

/* ------------------------------------------------------- AI theme modal */
async function generateTheme() {
  if (jobBusy.theme) return;
  const desc = $("theme-desc").value.trim();
  if (!desc) { toast(lang === "zh-CN" ? "请先描述想要的风格" : "Describe the style first", "err"); return; }
  if (!(await hasProvider())) { toast(t("theme_need_ai"), "err"); return; }
  $("theme-status").textContent = "";
  jobBusy.theme = true;
  try {
    const job = await AIJobs.run("theme", { description: desc }, { button: $("btn-theme-generate") });
    if (job.stopped) { $("theme-status").textContent = t("job_stopped"); return; }
    const res = job.result;
    // refresh theme list and apply
    const themes = await invoke("list_themes");
    $("theme-select").innerHTML = themes
      .map((x) => `<option value="${x.id}">${escapeHtml(lang === "zh-CN" ? x.name_zh || x.name : x.name)}</option>`)
      .join("");
    currentTheme = res.id;
    $("theme-select").value = currentTheme;
    localStorage.setItem("wxwright-theme", currentTheme);
    toast(t("theme_done", res.name_zh || res.name), "ok");
    window.Mozai && Mozai.celebrate();
    convertNow();
    if (res.warnings && res.warnings.length > 0) {
      toast((lang === "zh-CN" ? "提示：" : "Warnings: ") + res.warnings.slice(0, 3).join("; "));
    }
  } catch (e) {
    $("theme-status").textContent = String(e);
    toast(String(e), "err");
  } finally {
    delete jobBusy.theme;
  }
}

/* ------------------------------------------------------- platform switch */
let PLATFORMS = [];
let currentPlatform = localStorage.getItem("wxwright-platform") || "wechat";
function currentPlatformSpec() {
  return PLATFORMS.find((p) => p.id === currentPlatform)
    || { id: "wechat", name_zh: "微信公众号", name_en: "WeChat MP", rich_text: true, image_note: false,
         presets: [["头图 1080×460（2.35:1）", 1080, 460], ["次图 1080×1080（1:1）", 1080, 1080],
                   ["小方图 500×500（1:1）", 500, 500], ["正文横图 1280×720（16:9）", 1280, 720],
                   ["正文竖图 1080×1440（3:4）", 1080, 1440], ["贴图 900×383", 900, 383], ["贴图 383×383", 383, 383]],
         note: "" };
}
function platformLabel(p) { return lang === "zh-CN" ? p.name_zh : p.name_en; }

/* the primary copy action is a per-platform artifact (PRD §16 export adapter) */
function refreshCopyButton() {
  const label = $("copy-label");
  if (!label) return;
  const id = currentPlatform;
  label.textContent = id === "wechat" ? t("copy_rich")
    : id === "zhihu" ? t("copy_md")
    : t("copy_caption");
}

/* official mark per platform: PLATFORM_LOGOS first, vendor library fallback */
function platformLogoTile(id, size = 16) {
  const L = (window.PLATFORM_LOGOS || {})[id] || (window.BRAND_LOGOS || {})[id];
  if (L && L.path) {
    const fg = L.dark ? "#1F2328" : "#FFFFFF";
    return `<span class="brand-tile" style="width:${size}px;height:${size}px;background:${L.color};border-radius:${Math.max(3, Math.round(size * 0.25))}px;"><svg width="${Math.round(size * 0.72)}" height="${Math.round(size * 0.72)}" viewBox="0 0 24 24" aria-hidden="true"><path fill="${fg}" d="${L.path}"/></svg></span>`;
  }
  const name = platformLabel(PLATFORMS.find((p) => p.id === id) || { name_zh: id, name_en: id });
  return `<span class="brand-tile" style="width:${size}px;height:${size}px;background:var(--border-strong);border-radius:${Math.max(3, Math.round(size * 0.25))}px;font-size:${Math.max(8, Math.round(size * 0.5))}px;">${escapeHtml(name.charAt(0).toUpperCase())}</span>`;
}

function renderPlatformOptions() {
  const spec = currentPlatformSpec();
  $("platform-trigger-logo").innerHTML = platformLogoTile(spec.id, 18);
  $("platform-trigger-name").textContent = platformLabel(spec);
  $("platform-trigger").title = spec.note || "";
  const menu = $("platform-menu");
  menu.innerHTML = PLATFORMS.map((p) => `
    <button type="button" class="pm-item${p.id === currentPlatform ? " active" : ""}" data-id="${escapeHtml(p.id)}">
      ${platformLogoTile(p.id, 20)}
      <span class="pm-name">${escapeHtml(platformLabel(p))}</span>
      <svg class="pm-check" viewBox="0 0 24 24"><path d="M20 6 9 17l-5-5" fill="none" stroke="currentColor" stroke-width="2"/></svg>
    </button>`).join("");
  menu.querySelectorAll(".pm-item").forEach((item) => {
    item.addEventListener("click", (e) => {
      e.stopPropagation();
      $("platform-menu").hidden = true;
      if (item.dataset.id === currentPlatform) return;
      applyPlatform(item.dataset.id);
      if (currentArticleId) {
        dirty = true; // platform is part of the article; persist on next save
        $("stat-saved").textContent = t("not_saved");
      }
      refreshCopyButton();
      convertNow(); // channel preview + rule table switch immediately
      toast((lang === "zh-CN" ? "已切换到：" : "Platform: ") + platformLabel(currentPlatformSpec()), "ok");
    });
  });
}

/* preset labels arrive zh-canonical from core and the demo fallback;
   translate the size vocabulary for EN instead of duplicating the tables */
function presetLabel(label) {
  if (lang === "zh-CN") return label;
  const map = [
    ["正文横图", "Landscape"], ["正文竖图", "Portrait"], ["小方图", "Small square"],
    ["头图", "Cover"], ["次图", "Square"], ["贴图", "Sticker"], ["封面", "Cover"],
    ["方图", "Square"], ["横图", "Landscape"], ["竖图", "Portrait"],
  ];
  let out = label;
  for (const [zh, en] of map) out = out.split(zh).join(en);
  return out;
}

/* rebuild poster + fit-studio preset options from the active platform */
function renderPlatformPresets() {
  const spec = currentPlatformSpec();
  if (!Array.isArray(spec.presets)) return;
  const posterSel = $("poster-preset");
  posterSel.innerHTML = spec.presets
    .map(([label, w, h]) => `<option value="${w}x${h}">${escapeHtml(presetLabel(label))}</option>`)
    .join("") + `<option value="custom">${t("custom_size")}</option>`;
  const fitSel = $("fit-preset");
  if (fitSel) {
    fitSel.innerHTML = spec.presets
      .map(([label, w, h]) => `<option value="${w}x${h}">${escapeHtml(presetLabel(label))}</option>`)
      .join("");
  }
  const imgsetBtn = $("btn-poster-imgset");
  if (imgsetBtn) imgsetBtn.hidden = !spec.image_note;
}

function applyPlatform(id) {
  currentPlatform = id;
  localStorage.setItem("wxwright-platform", id);
  renderPlatformOptions();
  renderPlatformPresets();
  refreshCopyButton();
  if (!$("modal-poster").hidden) {
    // untouched template follows the new platform's canvas; edited HTML stays
    if (!posterTemplateDirty) {
      const { w, h } = posterSize();
      $("poster-html").value = posterTemplate(w, h, currentTpl);
      $("poster-custom").hidden = $("poster-preset").value !== "custom";
    }
    updatePosterPreview();
  }
}

async function initPlatformSwitcher() {
  if (invoke) {
    try { PLATFORMS = await invoke("list_platforms"); } catch (e) { PLATFORMS = []; }
  }
  if (!PLATFORMS.length) {
    // browser demo fallback: full platform set so every channel shell is
    // reachable without the desktop shell (same shape as core)
    PLATFORMS = [
      { id: "wechat", name_zh: "微信公众号", name_en: "WeChat MP", rich_text: true, image_note: false, note: "",
        presets: [["头图 1080×460（2.35:1）", 1080, 460], ["次图 1080×1080（1:1）", 1080, 1080],
                  ["小方图 500×500（1:1）", 500, 500], ["正文横图 1280×720（16:9）", 1280, 720],
                  ["正文竖图 1080×1440（3:4）", 1080, 1440], ["贴图 900×383", 900, 383], ["贴图 383×383", 383, 383]] },
      { id: "xhs", name_zh: "小红书", name_en: "Xiaohongshu", rich_text: false, image_note: true, note: "image-note",
        presets: [["封面 1080×1440（3:4）", 1080, 1440], ["方图 1080×1080（1:1）", 1080, 1080]] },
      { id: "zhihu", name_zh: "知乎", name_en: "Zhihu", rich_text: true, image_note: false, note: "markdown",
        presets: [["封面 1920×1080（16:9）", 1920, 1080]] },
      { id: "meta", name_zh: "Facebook", name_en: "Facebook", rich_text: false, image_note: false, note: "caption",
        presets: [["横图 1200×630（1.91:1）", 1200, 630], ["方图 1080×1080（1:1）", 1080, 1080], ["竖图 1080×1350（4:5）", 1080, 1350]] },
      { id: "instagram", name_zh: "Instagram", name_en: "Instagram", rich_text: false, image_note: true, note: "image-first",
        presets: [["竖图 1080×1350（4:5）", 1080, 1350], ["方图 1080×1080（1:1）", 1080, 1080], ["Story 1080×1920（9:16）", 1080, 1920]] },
      { id: "x", name_zh: "X (Twitter)", name_en: "X (Twitter)", rich_text: false, image_note: false, note: "caption-280",
        presets: [["横图 1600×900（16:9）", 1600, 900], ["方图 1080×1080（1:1）", 1080, 1080]] },
      { id: "linkedin", name_zh: "LinkedIn", name_en: "LinkedIn", rich_text: false, image_note: false, note: "caption",
        presets: [["横图 1200×627", 1200, 627], ["方图 1080×1080（1:1）", 1080, 1080]] },
    ];
  }
  if (!PLATFORMS.some((p) => p.id === currentPlatform)) currentPlatform = "wechat";
  renderPlatformOptions();
  renderPlatformPresets();
  refreshCopyButton();
  $("platform-trigger").addEventListener("click", (e) => {
    e.stopPropagation();
    const menu = $("platform-menu");
    if (menu.hidden) {
      renderPlatformOptions(); // refresh active state
      const btn = $("platform-trigger");
      const br = btn.getBoundingClientRect();
      menu.style.visibility = "hidden";
      menu.style.display = "block";
      const mw = menu.offsetWidth || 220;
      const mh = menu.offsetHeight || 240;
      menu.style.display = "";
      menu.style.visibility = "";
      menu.style.left = `${Math.round(Math.min(Math.max(8, br.left), window.innerWidth - mw - 8))}px`;
      menu.style.top = `${Math.round(Math.min(br.bottom + 6, window.innerHeight - mh - 8))}px`;
      menu.hidden = false;
    } else {
      menu.hidden = true;
    }
  });
  document.addEventListener("click", (e) => {
    const menu = $("platform-menu");
    if (!menu.hidden && !e.target.closest("#platform-menu") && !e.target.closest("#platform-trigger")) {
      menu.hidden = true;
    }
  });
}

/* --------------------------------------------------------- poster studio */
let posterTemplateDirty = false; // user/AI edited: don't auto-regenerate on platform switch
function posterSize() {
  const v = $("poster-preset").value;
  if (v === "custom") {
    return { w: Math.max(100, Math.min(4096, parseInt($("poster-w").value) || 800)), h: Math.max(100, Math.min(4096, parseInt($("poster-h").value) || 600)), scene: "自定义海报" };
  }
  const spec = currentPlatformSpec();
  const hit = spec.presets.find(([label, w, h]) => `${w}x${h}` === v);
  if (hit) return { w: hit[1], h: hit[2], scene: hit[0] };
  return { w: 1080, h: 1440, scene: "封面" };
}
function posterTemplate(w, h, tpl = "cover") {
  if (tpl === "quote") return posterQuoteCard(w, h);
  if (tpl === "pic") return posterPictureCard(w, h);
  const zh = lang === "zh-CN";
  return `<!doctype html>
<html><head><meta charset="utf-8"><style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body { width: ${w}px; height: ${h}px; overflow: hidden;
    font-family: "Noto Sans SC", "Microsoft YaHei UI", sans-serif; }
  .hero {
    width: 100%; height: 100%;
    background: linear-gradient(135deg, #2F6CEA 0%, #6C9BFF 60%, #EFF4FE 100%);
    display: flex; flex-direction: column; justify-content: center;
    padding: 0 ${Math.round(w * 0.08)}px; color: #fff;
  }
  .kicker { font-size: ${Math.round(h * 0.05)}px; letter-spacing: 4px; opacity: 0.85; }
  .title { font-size: ${Math.round(h * 0.16)}px; font-weight: 700; margin: ${Math.round(h * 0.03)}px 0; }
  .sub { font-size: ${Math.round(h * 0.07)}px; opacity: 0.92; }
  .sign { position: absolute; right: ${Math.round(w * 0.05)}px; bottom: ${Math.round(h * 0.07)}px;
    font-size: ${Math.round(h * 0.045)}px; opacity: 0.85; }
</style></head>
<body>
  <div class="hero">
    <div class="kicker">WXWRIGHT STUDIO</div>
    <div class="title">${zh ? "在这里写下你的标题" : "Write your title here"}</div>
    <div class="sub">${zh ? "副标题：一句话讲清楚这篇文章" : "One sentence that sells the article"}</div>
    <div class="sign">AI瑶 · 公众号 码聋</div>
  </div>
</body></html>`;
}
function posterQuoteCard(w, h) {
  return `<!doctype html>
<html><head><meta charset="utf-8"><style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body { width: ${w}px; height: ${h}px; overflow: hidden;
    font-family: "Noto Sans SC", "Microsoft YaHei UI", sans-serif; }
  .card { width: 100%; height: 100%; background: #FBF7F0;
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    padding: 0 ${Math.round(w * 0.1)}px; position: relative; }
  .mark { font-size: ${Math.round(h * 0.14)}px; color: #B45309; font-weight: 700; line-height: 1; }
  .quote { font-size: ${Math.round(h * 0.062)}px; color: #292524; line-height: 1.9;
    text-align: center; margin: ${Math.round(h * 0.04)}px 0; }
  .rule { width: ${Math.round(w * 0.14)}px; height: 3px; background: #B45309; margin: ${Math.round(h * 0.035)}px auto; }
  .sign { position: absolute; bottom: ${Math.round(h * 0.06)}px;
    font-size: ${Math.round(h * 0.032)}px; color: #A8A29E; letter-spacing: 2px; }
</style></head>
<body><div class="card">
  <div class="mark">"</div>
  <div class="quote">写作是把噪音调成静音，<br>留白是文章的呼吸。</div>
  <div class="rule"></div>
  <div class="sign">AI瑶 · 公众号 码聋</div>
</div></body></html>`;
}

function posterPictureCard(w, h) {
  return `<!doctype html>
<html><head><meta charset="utf-8"><style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body { width: ${w}px; height: ${h}px; overflow: hidden;
    font-family: "Noto Sans SC", "Microsoft YaHei UI", sans-serif; }
  .card { width: 100%; height: 100%; background: #FFFFFF;
    display: flex; flex-direction: column; }
  .pic { flex: 1 1 auto; background: #EFF4FE; display: flex; align-items: center;
    justify-content: center; color: #2F6CEA; font-size: ${Math.round(w * 0.035)}px;
    /* 把下面的占位换成 data URI 图片：<img src="data:image/png;base64,..."> */
  }
  .bar { flex: 0 0 auto; padding: ${Math.round(h * 0.045)}px ${Math.round(w * 0.07)}px;
    background: #FFFFFF; border-top: 1px solid #E4E7EC; }
  .t1 { font-size: ${Math.round(w * 0.045)}px; font-weight: 700; color: #1F2328; }
  .t2 { font-size: ${Math.round(w * 0.028)}px; color: #57606A; margin-top: ${Math.round(h * 0.012)}px; }
  .sign { position: absolute; right: ${Math.round(w * 0.05)}px; bottom: ${Math.round(h * 0.025)}px;
    font-size: ${Math.round(w * 0.022)}px; color: #8B949E; }
</style></head>
<body><div class="card" style="position: relative;">
  <div class="pic">图片区域：将 img.src 换成 data URI 或在海报工坊中改写</div>
  <div class="bar">
    <div class="t1">在这里写贴图标题</div>
    <div class="t2">一行说明文字，放在图片下方更聚焦</div>
  </div>
  <div class="sign">AI瑶 · 公众号 码聋</div>
</div></body></html>`;
}

function updatePosterPreview() {
  const { w, h } = posterSize();
  const iframe = $("poster-preview");
  iframe.srcdoc = $("poster-html").value;
  iframe.style.width = w + "px";
  iframe.style.height = h + "px";
  const box = $("poster-preview-box");
  const bw = (box.clientWidth || 460) - 20;
  const bh = (box.clientHeight || 350) - 20;
  const scale = Math.min(1, bw / w, bh / h);
  iframe.style.transform = `translate(-50%, -50%) scale(${scale})`;
  $("poster-preview-meta").textContent = `${w} × ${h} px · 预览 ${(scale * 100).toFixed(0)}%`;
}
/* ------------------------- channel preview shells (PRD §16) ----------------
   One article, one shape per channel - pixel-level, not recoloured: WeChat
   keeps the dialect article scroll; each social platform gets its own
   faithful feed layout (XHS note detail, X post, Facebook card, Instagram
   post, LinkedIn card) with its real ratios, type scale and action bars;
   Zhihu gets a Zhihu article page. Unknown ids fall back to the generic
   note shell. All inside the same phone mockup. */
const CHANNEL_ACCENT = { xhs: "#FF2442", meta: "#1877F2", instagram: "#E4405F", x: "#1D9BF0", linkedin: "#0A66C2", zhihu: "#0084FF" };
function channelAccent(id) { return CHANNEL_ACCENT[id] || "#2F6CEA"; }
function channelName(id) {
  const spec = PLATFORMS.find((p) => p.id === id);
  return spec ? platformLabel(spec) : "微信";
}

function noteShellHtml(model) {
  const accent = channelAccent(model.platform);
  const imgs = (model.images || []).filter(Boolean);
  const title = model.title || "";
  // caption: escape, keep line breaks, highlight #话题#
  let cap = escapeHtml(model.caption || "");
  cap = cap.replace(/#[^#\n]{1,40}#/g, (m) => `<span style="color: ${accent};">${m}</span>`);
  cap = cap.replace(/\n/g, "<br/>");
  // XHS official image ratio: 3:4 (1080x1440). Every card honours it.
  const strip = imgs.length
    ? `<div style="display: flex; gap: 8px; overflow-x: auto; scroll-snap-type: x mandatory; padding: 0 12px;">
        ${imgs.slice(0, 9).map((src, i) => `<img src="${src}" style="scroll-snap-align: start; flex: 0 0 100%; width: 100%; aspect-ratio: 3 / 4; object-fit: cover; border-radius: 12px;" />`).join("")}
      </div>
      <div style="display: flex; gap: 4px; justify-content: center; margin-top: 8px;">
        ${imgs.slice(0, Math.min(9, imgs.length)).map((_, i) => `<span style="width: 5px; height: 5px; border-radius: 50%; background: ${i === 0 ? accent : "#D9DDE3"};"></span>`).join("")}
      </div>`
    : `<div style="margin: 0 12px; aspect-ratio: 3 / 4; border-radius: 12px; background: linear-gradient(135deg, ${accent}, ${accent}BB); color: #fff; display: flex; align-items: center; justify-content: center; text-align: center; padding: 24px 20px; font-size: 24px; font-weight: 700; line-height: 1.4; box-sizing: border-box;">${escapeHtml(title)}</div>`;
  const heart = `<svg viewBox="0 0 24 24" width="19" height="19"><path fill="${accent}" d="M12 21s-7.5-4.7-10-9.3C.6 8.6 2.4 5 6 5c2.2 0 3.6 1.2 4.5 2.5h3C14.4 6.2 15.8 5 18 5c3.6 0 5.4 3.6 4 6.7C19.5 16.3 12 21 12 21Z" transform="scale(0.92) translate(1,0)"/></svg>`;
  const star = `<svg viewBox="0 0 24 24" width="19" height="19"><path fill="none" stroke="#57606A" stroke-width="1.8" d="m12 3 2.7 5.7 6.3.8-4.6 4.3 1.2 6.2L12 17l-5.6 3 1.2-6.2L3 9.5l6.3-.8L12 3Z"/></svg>`;
  const bubble = `<svg viewBox="0 0 24 24" width="19" height="19"><path fill="none" stroke="#57606A" stroke-width="1.8" d="M21 12a8 8 0 0 1-8 8H4l2.5-3A8 8 0 1 1 21 12Z"/></svg>`;
  return `<!doctype html><html><head><meta charset="utf-8"><style>
    html,body{margin:0;padding:0;background:#fff;}
    body{font-family:-apple-system,"PingFang SC","Microsoft YaHei UI",sans-serif;color:#1F2328;padding-top:26px;}
    .bar{display:flex;align-items:center;gap:6px;padding:8px 14px;border-bottom:1px solid #F0F1F3;}
    .bar b{font-size:14px;}
    .bar .follow{margin-left:auto;background:${accent};color:#fff;font-size:12px;padding:4px 14px;border-radius:999px;}
    .wrap{padding:12px 14px 20px;}
    .title{font-size:18px;font-weight:700;line-height:1.45;margin:10px 0 6px;}
    .author{display:flex;align-items:center;gap:8px;margin:12px 0;}
    .avatar{width:30px;height:30px;border-radius:50%;background:linear-gradient(135deg,#2F6CEA,#7A50EC);color:#fff;font-size:13px;font-weight:700;display:flex;align-items:center;justify-content:center;}
    .author .n{font-size:13px;font-weight:600;}
    .author .sub{font-size:11px;color:#8B949E;}
    .author .follow{margin-left:auto;font-size:12px;color:${accent};border:1px solid ${accent};padding:3px 12px;border-radius:999px;}
    .cap{font-size:14.5px;line-height:1.85;word-break:break-word;}
    .actions{display:flex;gap:22px;align-items:center;border-top:1px solid #F0F1F3;margin-top:14px;padding-top:10px;color:#57606A;font-size:12px;}
    .actions span{display:flex;align-items:center;gap:5px;}
    ::-webkit-scrollbar{width:0;height:0;display:none;}
  </style></head><body>
    <div class="bar"><b>${escapeHtml(channelName(model.platform))}</b><span class="follow">打开</span></div>
    <div style="margin-top: 10px;">${strip}</div>
    <div class="wrap">
      <div class="title">${escapeHtml(title)}</div>
      <div class="author">
        <div class="avatar">瑶</div>
        <div><div class="n">AI瑶</div><div class="sub">公众号 码聋</div></div>
        <div class="follow">关注</div>
      </div>
      <div class="cap">${cap}</div>
      <div class="actions">${heart}<span>1.2k</span>${star}<span>856</span>${bubble}<span>45</span></div>
    </div>
  </body></html>`;
}

/* -- shared shell primitives: icons / status bar / avatar / caption / media -- */
const SHELL_ICONS = {
  heart: "M12 20.7C7.2 17.3 3 13.9 3 9.9 3 7.2 5 5 7.6 5c1.8 0 3.3 1 4.4 2.6C13.1 6 14.6 5 16.4 5 19 5 21 7.2 21 9.9c0 4-4.2 7.4-9 10.8Z",
  star: "m12 3.6 2.5 5.1 5.7.8-4.1 4 1 5.6-5.1-2.7-5.1 2.7 1-5.6-4.1-4 5.7-.8L12 3.6Z",
  chat: "M21 11.5a8.5 8.5 0 0 1-8.5 8.5H4l2.2-2.6A8.5 8.5 0 1 1 21 11.5Z",
  repost: "M17 3.5 20.5 7 17 10.5M20.5 7H8.5a4 4 0 0 0-4 4v1.5M7 20.5 3.5 17 7 13.5M3.5 17h12a4 4 0 0 0 4-4v-1.5",
  send: "M22 2 11 13M22 2l-7 20-4-9-9-4 20-7Z",
  bookmark: "M6 3.5h12a.5.5 0 0 1 .5.5v16.5L12 16l-6.5 4.5V4a.5.5 0 0 1 .5-.5Z",
  thumb: "M7 10.5V21H4.5A1.5 1.5 0 0 1 3 19.5V12a1.5 1.5 0 0 1 1.5-1.5H7Zm0 0 3.8-6.7A2.3 2.3 0 0 1 13.1 6v3h4.6a2 2 0 0 1 2 2.4l-1.1 5.6a2 2 0 0 1-2 1.5H7",
  globe: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-18c-2.5 2.3-4 5.5-4 9s1.5 6.7 4 9c2.5-2.3 4-5.5 4-9s-1.5-6.7-4-9ZM3.4 9h17.2M3.4 15h17.2",
  back: "M15 4.5 7.5 12l7.5 7.5",
  chart: "M4.5 20v-9m6 9V4m6 16v-6",
  search: "M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13Zm9.5 3.5L15.5 16",
  dots: "M5 12h.01M12 12h.01M19 12h.01",
};
function shellIcon(name, size, color, opts = {}) {
  const fill = opts.filled ? color : "none";
  const stroke = opts.filled ? "none" : color;
  return `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="${fill}" stroke="${stroke}" stroke-width="${opts.sw || 1.7}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${SHELL_ICONS[name]}"/></svg>`;
}
/* the phone mock already draws the status bar + notch over the screen; shells
   only reserve clear space for it */
function shellTop(bg) {
  return `<div style="height:34px;background:${bg};"></div>`;
}
function shellAvatar(size, igRing) {
  const inner = `<span style="width:100%;height:100%;border-radius:50%;background:linear-gradient(135deg,#2F6CEA,#7A50EC);color:#fff;font-size:${Math.round(size * 0.4)}px;font-weight:700;display:flex;align-items:center;justify-content:center;box-sizing:border-box;">瑶</span>`;
  if (igRing) {
    return `<span style="flex:none;width:${size}px;height:${size}px;border-radius:50%;padding:2px;background:linear-gradient(45deg,#F58529,#DD2A7B,#8134AF);box-sizing:border-box;">${inner}</span>`;
  }
  return `<span style="flex:none;width:${size}px;height:${size}px;border-radius:50%;overflow:hidden;">${inner}</span>`;
}
function shellCaption(caption, tagColor) {
  let cap = escapeHtml(caption || "");
  if (tagColor !== null) {
    cap = cap.replace(/#[^#\s]{1,40}(#|(?=\s|$))/g, (m) => `<span style="color:${tagColor};font-weight:600;">${m}</span>`);
  }
  return cap.replace(/\n/g, "<br/>");
}
function shellMedia(imgs, ratio, fallbackTitle, accent, caption) {
  if (imgs && imgs.length) {
    return `<img src="${imgs[0]}" alt="" style="width:100%;display:block;aspect-ratio:${ratio};object-fit:cover;" />`;
  }
  return `<div style="width:100%;aspect-ratio:${ratio};background:linear-gradient(135deg,${accent},${accent}B3);color:#fff;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;text-align:center;padding:20px;box-sizing:border-box;">
    <div style="font-size:20px;font-weight:700;line-height:1.4;">${escapeHtml(fallbackTitle || "")}</div>
    <div style="font-size:12px;opacity:0.85;max-width:90%;">${escapeHtml((caption || "").slice(0, 60))}</div>
  </div>`;
}
function shellDoc(css, body) {
  return `<!doctype html><html><head><meta charset="utf-8"><style>
    html,body{margin:0;padding:0;}
    body{font-family:-apple-system,"PingFang SC","Microsoft YaHei UI","Segoe UI",sans-serif;background:#fff;-webkit-font-smoothing:antialiased;}
    ::-webkit-scrollbar{width:0;height:0;display:none;}
    ${css}
  </style></head><body>${body}</body></html>`;
}
function shellStr() {
  const zh = {
    follow: "关注", like: "赞", comment: "评论", share: "分享", repost: "转发",
    send: "发送", upvote: "赞同", say: "说点什么...", search: "搜索",
    post: "帖子", article: "文章", bio: "公众号 码聋", persona: "AI瑶",
    handle: "@aiyao_coder", headline: "AI 内容创作者 · 公众号「码聋」",
    justNow: "刚刚", date: "09-25", edited: "编辑于 09-25",
    views: "14.2万 次浏览", likesCount: "1,284 次赞", viewAll: "查看全部 45 条评论",
    reactionN: "128", fbCounts: "12 条评论 · 3 次分享", liCounts: "45 条评论 · 12 次转发",
    copyright: "著作权归作者所有",
  };
  return lang === "zh-CN" ? zh : {
    follow: "Follow", like: "Like", comment: "Comment", share: "Share", repost: "Repost",
    send: "Send", upvote: "Upvote", say: "Say something...", search: "Search",
    post: "Post", article: "Article", bio: "AI content creator", persona: "AI Yao",
    handle: "@aiyao_coder", headline: "AI content creator · WeChat: CodingDeaf",
    justNow: "now", date: "Sep 25", edited: "Edited Sep 25",
    views: "142K views", likesCount: "1,284 likes", viewAll: "View all 45 comments",
    reactionN: "128", fbCounts: "12 comments · 3 shares", liCounts: "45 comments · 12 reposts",
    copyright: "All rights reserved",
  };
}

/* 1) 小红书笔记详情页: 全出血 3:4 封面轮播 + 圆点 + 图上悬浮操作列(点赞/收藏/评论
   白圈) + 16px 粗标题 + 话题红标 + 底部固定条(头像/昵称/红色关注 + 说点什么 + 心/星) */
function xhsNoteShell(model) {
  const S = shellStr();
  const accent = "#FF2442";
  const imgs = (model.images || []).filter(Boolean);
  // the caption's first line repeats the title (render_caption); XHS shows a
  // separate title field, so drop the duplicate
  let cap = model.caption || "";
  if (model.title && cap.startsWith(model.title)) cap = cap.slice(model.title.length).replace(/^\n+/, "");
  const overlay = imgs.length
    ? `<div style="position:absolute;bottom:14px;left:50%;transform:translateX(-50%);display:flex;gap:5px;">
        ${imgs.slice(0, 9).map((_, i) => `<i style="width:6px;height:6px;border-radius:50%;background:${i === 0 ? "#fff" : "rgba(255,255,255,0.45)"};display:block;"></i>`).join("")}
      </div>
      <div style="position:absolute;right:10px;bottom:14px;display:flex;flex-direction:column;gap:9px;">
        ${[["heart", "1.2k", true], ["star", "856", false], ["chat", "45", false]].map(([n, c, hot]) => `
          <span style="display:flex;flex-direction:column;align-items:center;gap:2px;">
            <span style="width:34px;height:34px;border-radius:50%;background:rgba(0,0,0,0.28);display:flex;align-items:center;justify-content:center;box-sizing:border-box;">${shellIcon(n, 18, hot ? accent : "#fff", { filled: !!hot })}</span>
            <b style="color:#fff;font-size:11px;font-weight:600;">${c}</b>
          </span>`).join("")}
      </div>`
    : "";
  return shellDoc("", `
    ${shellTop("#FFFFFF")}
    <div style="position:relative;">
      ${shellMedia(imgs, "3 / 4", model.title, accent, cap)}
      ${overlay}
    </div>
    <div style="padding:12px 14px 0;">
      <div style="font-size:16px;font-weight:700;color:#333;line-height:1.45;">${escapeHtml(model.title || "")}</div>
      <div style="font-size:15px;color:#333;line-height:1.7;margin-top:8px;">${shellCaption(cap, accent)}</div>
      <div style="font-size:12px;color:#999;margin-top:10px;">${S.edited}</div>
    </div>
    <div style="height:64px;"></div>
    <div style="position:fixed;left:0;right:0;bottom:0;background:#fff;border-top:1px solid #F3F3F3;display:flex;align-items:center;gap:8px;padding:8px 12px;">
      ${shellAvatar(30)}
      <span style="font-size:13px;font-weight:600;color:#333;flex:none;">${S.persona}</span>
      <span style="background:${accent};color:#fff;font-size:12px;font-weight:600;padding:3px 12px;border-radius:999px;flex:none;">${S.follow}</span>
      <span style="flex:1;"></span>
      <span style="background:#F5F5F5;color:#999;font-size:12px;padding:6px 12px;border-radius:999px;flex:none;">${S.say}</span>
      ${shellIcon("heart", 19, "#333", { filled: true })}
      <b style="font-size:12px;color:#333;font-weight:600;">1.2k</b>
      ${shellIcon("star", 19, "#333")}
      <b style="font-size:12px;color:#333;font-weight:600;">856</b>
    </div>`);
}

/* 2) X (Twitter): 左侧 40px 头像列 + 昵称/句柄/时间一行 + 15px 正文 + 16:9 圆角16px
   媒体 + 时间/浏览行 + 指标行(回复/转发/红心/浏览 + 书签/私信), 分隔线 #EFF3F4 */
function xPostShell(model) {
  const S = shellStr();
  const imgs = (model.images || []).filter(Boolean);
  return shellDoc("", `
    ${shellTop("#FFFFFF")}
    <div style="display:flex;align-items:center;gap:20px;padding:10px 16px;border-bottom:1px solid #EFF3F4;">
      ${shellIcon("back", 19, "#0F1419")}
      <b style="font-size:17px;color:#0F1419;">${S.post}</b>
    </div>
    <div style="display:flex;gap:12px;padding:12px 16px 6px;">
      ${shellAvatar(40)}
      <div style="flex:1;min-width:0;">
        <div style="display:flex;align-items:center;gap:4px;">
          <b style="font-size:15px;color:#0F1419;">${S.persona}</b>
          <span style="width:15px;height:15px;border-radius:50%;background:#1D9BF0;display:inline-flex;align-items:center;justify-content:center;flex:none;">
            <svg viewBox="0 0 24 24" width="9" height="9" fill="#fff"><path d="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2Z"/></svg>
          </span>
          <span style="font-size:15px;color:#536471;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${S.handle} · ${S.justNow}</span>
          <span style="margin-left:auto;flex:none;">${shellIcon("dots", 15, "#536471", { sw: 2.6 })}</span>
        </div>
        <div style="font-size:15px;line-height:1.4;color:#0F1419;margin:2px 0 12px;">${shellCaption(model.caption, "#1D9BF0")}</div>
        ${imgs.length ? `<div style="border:1px solid #EFF3F4;border-radius:16px;overflow:hidden;margin-bottom:12px;">${shellMedia(imgs, "16 / 9", model.title, "#1D9BF0", model.caption)}</div>` : ""}
        <div style="font-size:15px;color:#536471;padding:2px 0 10px;border-bottom:1px solid #EFF3F4;">${S.date} · ${S.views}</div>
        <div style="display:flex;justify-content:space-between;align-items:center;padding:12px 2px 4px;">
          <span style="display:flex;align-items:center;gap:6px;font-size:13px;color:#536471;">${shellIcon("chat", 18, "#536471")}45</span>
          <span style="display:flex;align-items:center;gap:6px;font-size:13px;color:#536471;">${shellIcon("repost", 18, "#536471")}128</span>
          <span style="display:flex;align-items:center;gap:6px;font-size:13px;color:#536471;">${shellIcon("heart", 18, "#F91880", { filled: true })}1,244</span>
          <span style="display:flex;align-items:center;gap:6px;font-size:13px;color:#536471;">${shellIcon("chart", 18, "#536471")}</span>
          <span style="display:flex;align-items:center;gap:14px;">${shellIcon("bookmark", 18, "#536471")}${shellIcon("send", 18, "#536471")}</span>
        </div>
      </div>
    </div>`);
}

/* 3) Facebook: #F0F2F5 页底 + 白色 8px 圆角卡片 + 蓝色 wordmark 顶栏 + 40px 头像
   卡头(姓名/时间+地球) + 全出血媒体 + 反应簇计数行 + 灰色三栏操作条(赞/评论/分享) */
function facebookShell(model) {
  const S = shellStr();
  const imgs = (model.images || []).filter(Boolean);
  return shellDoc("", `
    ${shellTop("#FFFFFF")}
    <div style="display:flex;align-items:center;gap:8px;padding:8px 14px;border-bottom:1px solid #E4E6EB;background:#fff;">
      <b style="font-size:21px;color:#1877F2;letter-spacing:-0.5px;">facebook</b>
      <span style="flex:1;"></span>
      <span style="width:32px;height:32px;border-radius:50%;background:#E4E6EB;display:flex;align-items:center;justify-content:center;box-sizing:border-box;">${shellIcon("search", 16, "#050505")}</span>
      <span style="width:32px;height:32px;border-radius:50%;background:#E4E6EB;display:flex;align-items:center;justify-content:center;box-sizing:border-box;">${shellIcon("chat", 16, "#050505")}</span>
    </div>
    <div style="background:#F0F2F5;padding:8px 0 16px;">
      <div style="background:#fff;border-radius:8px;margin:0 8px;box-shadow:0 1px 2px rgba(0,0,0,0.12);overflow:hidden;">
        <div style="display:flex;align-items:center;gap:8px;padding:12px 14px 8px;">
          ${shellAvatar(40)}
          <div style="flex:1;min-width:0;">
            <div style="font-size:15px;font-weight:600;color:#050505;">${S.persona}</div>
            <div style="display:flex;align-items:center;gap:4px;font-size:13px;color:#65676B;">${S.justNow} · ${shellIcon("globe", 11, "#65676B")}</div>
          </div>
          ${shellIcon("dots", 17, "#65676B", { sw: 2.6 })}
        </div>
        <div style="font-size:15px;line-height:1.35;color:#050505;padding:2px 14px 10px;">${shellCaption(model.caption, null)}</div>
        ${imgs.length ? shellMedia(imgs, "4 / 5") : ""}
        <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 14px;">
          <span style="display:flex;align-items:center;">
            <span style="width:20px;height:20px;border-radius:50%;background:#1877F2;display:flex;align-items:center;justify-content:center;border:1.5px solid #fff;box-sizing:border-box;">${shellIcon("thumb", 11, "#fff", { filled: true })}</span>
            <span style="width:20px;height:20px;border-radius:50%;background:#F33E58;display:flex;align-items:center;justify-content:center;border:1.5px solid #fff;margin-left:-6px;box-sizing:border-box;">${shellIcon("heart", 11, "#fff", { filled: true })}</span>
            <b style="font-size:15px;color:#65676B;font-weight:400;margin-left:6px;">${S.reactionN}</b>
          </span>
          <span style="font-size:15px;color:#65676B;">${S.fbCounts}</span>
        </div>
        <div style="display:flex;border-top:1px solid #E4E6EB;">
          ${[["thumb", S.like], ["chat", S.comment], ["send", S.share]].map(([n, l]) => `
            <span style="flex:1;display:flex;align-items:center;justify-content:center;gap:7px;height:42px;font-size:15px;font-weight:600;color:#65676B;">${shellIcon(n, 17, "#65676B")}${l}</span>`).join("")}
        </div>
      </div>
    </div>`);
}

/* 4) Instagram: 手写体 wordmark 顶栏 + 渐变故事环头像 + 无圆角 4:5 大图 + 图下
   黑描边操作行(心/评论/私信 左, 书签 右) + 次赞粗体 + 用户名前缀 caption + 评论摘要 */
function instagramShell(model) {
  const S = shellStr();
  const imgs = (model.images || []).filter(Boolean);
  return shellDoc("", `
    ${shellTop("#FFFFFF")}
    <div style="display:flex;align-items:center;gap:14px;padding:8px 14px;border-bottom:1px solid #DBDBDB;background:#fff;">
      <b style="font-size:20px;font-family:'Segoe Script','Brush Script MT',cursive;font-weight:700;color:#000;">Instagram</b>
      <span style="flex:1;"></span>
      ${shellIcon("heart", 22, "#000")}
      ${shellIcon("send", 22, "#000")}
    </div>
    <div style="display:flex;align-items:center;gap:10px;padding:8px 12px;">
      ${shellAvatar(32, true)}
      <b style="font-size:13px;color:#000;">${S.persona}</b>
      <span style="margin-left:auto;">${shellIcon("dots", 15, "#000", { sw: 2.6 })}</span>
    </div>
    ${shellMedia(imgs, "4 / 5", model.title, "#DD2A7B", model.caption)}
    <div style="display:flex;align-items:center;gap:16px;padding:10px 12px 4px;">
      ${shellIcon("heart", 24, "#000")}
      ${shellIcon("chat", 24, "#000")}
      ${shellIcon("send", 24, "#000")}
      <span style="flex:1;"></span>
      ${shellIcon("bookmark", 24, "#000")}
    </div>
    <div style="padding:4px 12px 12px;">
      <b style="font-size:14px;color:#000;">${S.likesCount}</b>
      <div style="font-size:14px;color:#000;line-height:1.5;margin-top:4px;"><b>${S.persona}</b> ${shellCaption(model.caption, "#00376B")}</div>
      <div style="font-size:14px;color:#8E8E8E;margin-top:6px;">${S.viewAll}</div>
      <div style="font-size:11px;color:#8E8E8E;margin-top:6px;letter-spacing:0.5px;">${S.date}</div>
    </div>`);
}

/* 5) LinkedIn: 暖灰 #F4F2EE 页底 + 白色 8px 圆角卡片 + 48px 头像/职位头衔/时间+
   地球 + 全出血 1.91:1 媒体 + 反应簇/评论转发计数 + 四栏操作条(赞/评论/转发/发送) */
function linkedinShell(model) {
  const S = shellStr();
  const imgs = (model.images || []).filter(Boolean);
  return shellDoc("", `
    ${shellTop("#FFFFFF")}
    <div style="display:flex;align-items:center;gap:10px;padding:8px 12px;border-bottom:1px solid #E8E7E4;background:#fff;">
      <span style="width:30px;height:30px;border-radius:5px;background:#0A66C2;color:#fff;font-size:16px;font-weight:800;display:flex;align-items:center;justify-content:center;flex:none;">in</span>
      <span style="flex:1;height:32px;border-radius:4px;background:#EDF3F8;display:flex;align-items:center;gap:6px;padding:0 10px;color:#56687A;font-size:13px;box-sizing:border-box;">${shellIcon("search", 14, "#56687A")}${S.search}</span>
    </div>
    <div style="background:#F4F2EE;padding:8px 0 16px;">
      <div style="background:#fff;border-radius:8px;margin:0 8px;border:1px solid #E8E7E4;overflow:hidden;">
        <div style="display:flex;align-items:flex-start;gap:10px;padding:12px 14px 6px;">
          ${shellAvatar(48)}
          <div style="flex:1;min-width:0;">
            <div style="font-size:14px;font-weight:600;color:rgba(0,0,0,0.9);">${S.persona}</div>
            <div style="font-size:12px;color:rgba(0,0,0,0.6);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${S.headline}</div>
            <div style="display:flex;align-items:center;gap:4px;font-size:12px;color:rgba(0,0,0,0.6);">${S.justNow} · ${shellIcon("globe", 10, "rgba(0,0,0,0.6)")}</div>
          </div>
          <span style="display:flex;align-items:center;gap:12px;flex:none;">
            <b style="color:#0A66C2;font-size:13px;font-weight:600;">+ ${S.follow}</b>
            ${shellIcon("dots", 15, "rgba(0,0,0,0.6)", { sw: 2.6 })}
          </span>
        </div>
        <div style="font-size:14px;line-height:1.45;color:rgba(0,0,0,0.9);padding:2px 14px 10px;">${shellCaption(model.caption, null)}</div>
        ${imgs.length ? shellMedia(imgs, "1.91 / 1") : ""}
        <div style="display:flex;align-items:center;justify-content:space-between;padding:8px 14px;">
          <span style="display:flex;align-items:center;">
            <span style="width:17px;height:17px;border-radius:50%;background:#378FE9;display:flex;align-items:center;justify-content:center;border:1.5px solid #fff;box-sizing:border-box;">${shellIcon("thumb", 9, "#fff", { filled: true })}</span>
            <span style="width:17px;height:17px;border-radius:50%;background:#6DAE4F;display:flex;align-items:center;justify-content:center;border:1.5px solid #fff;margin-left:-5px;box-sizing:border-box;">${shellIcon("star", 9, "#fff", { filled: true })}</span>
            <b style="font-size:12px;color:#56687A;font-weight:400;margin-left:6px;">256</b>
          </span>
          <span style="font-size:12px;color:#56687A;">${S.liCounts}</span>
        </div>
        <div style="display:flex;border-top:1px solid #E8E7E4;">
          ${[["thumb", S.like], ["chat", S.comment], ["repost", S.repost], ["send", S.send]].map(([n, l]) => `
            <span style="flex:1;display:flex;align-items:center;justify-content:center;gap:6px;height:44px;font-size:13px;font-weight:600;color:#56687A;">${shellIcon(n, 17, "#56687A")}${l}</span>`).join("")}
        </div>
      </div>
    </div>`);
}

/* 6) 知乎文章页: 白底 + 返回/文章/蓝色关注顶栏 + 标题 21px + 作者行(34px 头像/
   一句话介绍) + 15px/1.67 两端对齐正文 + 底部蓝底赞同按钮 + 喜欢/收藏/评论 */
function zhihuArticleShell(html) {
  const S = shellStr();
  let body = html || "";
  let title = "";
  const h1 = body.match(/^\s*<h1>([\s\S]*?)<\/h1>/i);
  if (h1) {
    title = h1[1];
    body = body.slice(h1[0].length);
  }
  return shellDoc(`
    body{background:#fff;color:#121212;}
    .art{padding:0 16px 90px;}
    .art h2{font-size:19px;font-weight:600;margin:20px 0 8px;}
    .art h3{font-size:17px;font-weight:600;margin:16px 0 6px;}
    .art p{font-size:15px;line-height:1.7;margin:0 0 14px;text-align:justify;}
    .art img{max-width:100%;border-radius:4px;display:block;margin:6px auto;}
    .art ul,.art ol{padding-left:22px;margin:0 0 14px;}
    .art li{font-size:15px;line-height:1.7;margin:4px 0;}
    .art blockquote{margin:10px 0;padding:8px 12px;background:#F6F6F6;color:#64645F;border-left:3px solid #D3D3D3;}
    .art pre{background:#F6F6F6;padding:12px;border-radius:4px;overflow-x:auto;font-size:13px;}
    .art table{border-collapse:collapse;width:100%;font-size:13px;margin:10px 0;}
    .art code{background:#F2F2F2;padding:1px 5px;border-radius:3px;font-size:13px;}
  `, `
    ${shellTop("#FFFFFF")}
    <div style="display:flex;align-items:center;gap:16px;padding:10px 16px;border-bottom:1px solid #F0F0F0;">
      ${shellIcon("back", 18, "#121212")}
      <b style="font-size:16px;color:#121212;">${S.article}</b>
      <span style="flex:1;"></span>
      <span style="background:#0084FF;color:#fff;font-size:13px;font-weight:600;padding:4px 14px;border-radius:3px;">${S.follow}</span>
    </div>
    <div class="art">
      <div style="font-size:21px;font-weight:700;line-height:1.4;color:#121212;margin:14px 0 12px;">${title}</div>
      <div style="display:flex;align-items:center;gap:10px;margin-bottom:6px;">
        ${shellAvatar(34)}
        <div style="flex:1;">
          <div style="font-size:14px;font-weight:600;color:#121212;">${S.persona}</div>
          <div style="font-size:12px;color:#8590A6;">${S.bio}</div>
        </div>
      </div>
      <div style="font-size:13px;color:#8590A6;margin-bottom:10px;">${S.date} · ${S.copyright}</div>
      ${body}
    </div>
    <div style="position:fixed;left:0;right:0;bottom:0;background:#fff;border-top:1px solid #F0F0F0;display:flex;align-items:center;gap:10px;padding:8px 12px;">
      <span style="background:#EBF3FF;color:#0084FF;font-size:14px;font-weight:600;padding:6px 16px;border-radius:3px;display:flex;align-items:center;gap:5px;">${shellIcon("thumb", 14, "#0084FF")}${S.upvote} 45</span>
      <span style="flex:1;"></span>
      ${[["heart", "67"], ["star", "89"], ["chat", "45"]].map(([n, c]) => `<span style="display:flex;align-items:center;gap:4px;color:#8590A6;font-size:13px;">${shellIcon(n, 18, "#8590A6")}${c}</span>`).join("")}
    </div>`);
}

/* dispatch: one model, the target platform's own shell */
const SHELL_BUILDERS = { xhs: xhsNoteShell, x: xPostShell, meta: facebookShell, instagram: instagramShell, linkedin: linkedinShell };
function renderChannelShell(platform, model) {
  const build = SHELL_BUILDERS[platform];
  if (build) return build(model);
  return noteShellHtml(model); // unknown caption platform: generic fallback
}
function demoCaptionFromMd(md) {
  return (md || "").split("\n")
    .filter((l) => !/^\s*(!\[|<|```|---|\|)/.test(l))
    .map((l) => l.replace(/^#{1,6}\s*/, "").replace(/\[([^\]]*)\]\([^)]*\)/g, "$1").replace(/[*`>~_]/g, "").replace(/^(\s*)[-*]\s+/, "$1• ").trim())
    .filter(Boolean)
    .join("\n\n");
}
/* demo-only minimal Markdown -> plain HTML so the Zhihu shell has a body */
function demoPlainHtmlFromMd(md) {
  const out = [];
  let inList = false;
  const esc = (s) => escapeHtml(s)
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
  const closeList = () => { if (inList) { out.push("</ul>"); inList = false; } };
  for (const raw of (md || "").split("\n")) {
    const l = raw.trim();
    if (/^(```|!\[|\||---)/.test(l)) continue;
    if (/^###\s/.test(l)) { closeList(); out.push(`<h3>${esc(l.slice(4))}</h3>`); }
    else if (/^##\s/.test(l)) { closeList(); out.push(`<h2>${esc(l.slice(3))}</h2>`); }
    else if (/^#\s/.test(l)) { closeList(); out.push(`<h1>${esc(l.slice(2))}</h1>`); }
    else if (/^[-*]\s+/.test(l)) { if (!inList) { out.push("<ul>"); inList = true; } out.push(`<li>${esc(l.replace(/^[-*]\s+/, ""))}</li>`); }
    else if (/^>\s?/.test(l)) { closeList(); out.push(`<blockquote>${esc(l.replace(/^>\s?/, ""))}</blockquote>`); }
    else if (l) { closeList(); out.push(`<p>${esc(l)}</p>`); }
  }
  closeList();
  return out.join("\n");
}

async function blobToBase64(blob) {
  return new Promise((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(String(r.result).split(",")[1]);
    r.onerror = rej;
    r.readAsDataURL(blob);
  });
}
async function htmlToPngBlob(html, w, h, scale) {
  const iframe = document.createElement("iframe");
  iframe.style.cssText = `position:fixed;left:-99999px;top:0;width:${w}px;height:${h}px;border:0;`;
  document.body.appendChild(iframe);
  await new Promise((res) => { iframe.onload = res; iframe.srcdoc = html; });
  await new Promise((r) => setTimeout(r, 150));
  const doc = iframe.contentDocument;
  const serialized = new XMLSerializer().serializeToString(doc.documentElement);
  iframe.remove();
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w * scale}" height="${h * scale}"><foreignObject width="${w}" height="${h}">${serialized}</foreignObject></svg>`;
  const img = new Image();
  const url = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
  await new Promise((res, rej) => {
    img.onload = res;
    img.onerror = () => rej(new Error(lang === "zh-CN" ? "光栅化失败：HTML 需自包含，不能引用外部资源" : "Rasterization failed: HTML must be self-contained"));
    img.src = url;
  });
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(w * scale);
  canvas.height = Math.round(h * scale);
  const ctx = canvas.getContext("2d");
  ctx.scale(scale, scale);
  ctx.drawImage(img, 0, 0, w, h);
  return await new Promise((res) => canvas.toBlob(res, "image/png"));
}
async function posterExport(insert) {
  if (!invoke) { toast(t("demo_mode"), "err"); return; }
  const { w, h } = posterSize();
  try {
    $("status-text").textContent = t("poster_rasterizing");
    const blob = await htmlToPngBlob($("poster-html").value, w, h, 1);
    const b64 = await blobToBase64(blob);
    const name = `poster-${w}x${h}-${Date.now()}.png`;
    const path = await invoke("import_image_bytes", { filename: name, base64Data: b64 });
    if (insert) {
      insertAtCursor(`\n![海报](${path})\n`);
      closeModal("modal-poster");
      toast(t("poster_inserted"), "ok");
      window.Mozai && Mozai.celebrate();
    } else {
      const { save } = window.__TAURI__.dialog;
      const target = await save({ title: "Save PNG", defaultPath: name, filters: [{ name: "PNG", extensions: ["png"] }] });
      if (target) {
        await invoke("write_file_base64", { path: target, base64Data: b64 });
        toast(t("poster_saved") + " " + target, "ok");
      }
    }
    $("status-text").textContent = t("ready");
  } catch (e) {
    $("status-text").textContent = t("ready");
    toast(String(e), "err");
  }
}
/* Xiaohongshu-style image-note export: rasterize the current poster at
   every preset size of the active platform (deduped) into the asset library. */
async function exportImageSet() {
  if (!invoke) { toast(t("demo_mode"), "err"); return; }
  const html = $("poster-html").value.trim();
  if (!html) { toast(lang === "zh-CN" ? "请先生成或粘贴海报 HTML" : "Generate or paste poster HTML first", "err"); return; }
  const spec = currentPlatformSpec();
  const seen = new Set();
  const sizes = spec.presets.filter(([, w, h]) => {
    const k = `${w}x${h}`;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
  $("status-text").textContent = t("poster_rasterizing");
  let n = 0;
  try {
    for (const [, w, h] of sizes) {
      try {
        const blob = await htmlToPngBlob(html, w, h, 1);
        const b64 = await blobToBase64(blob);
        await invoke("import_image_bytes", {
          filename: `${spec.id}-set-${w}x${h}-${Date.now()}.png`,
          base64Data: b64,
        });
        n++;
      } catch (e) {
        toast(`${w}×${h}: ${e}`, "err");
      }
    }
    if (n > 0) {
      toast(t("imgset_done", n), "ok");
      window.Mozai && Mozai.celebrate();
    }
  } finally {
    delete jobBusy.poster;
    $("status-text").textContent = t("ready");
  }
}

async function posterAiGenerate() {
  if (jobBusy.poster) return;
  if (!(await hasProvider())) { toast(t("poster_need_ai"), "err"); return; }
  const { w, h, scene } = posterSize();
  const desc = $("poster-desc").value.trim();
  if (!desc) { toast(lang === "zh-CN" ? "请先描述海报内容与风格" : "Describe the poster first", "err"); return; }
  $("status-text").textContent = t("poster_generating");
  jobBusy.poster = true;
  try {
    const job = await AIJobs.run(
      "poster",
      { description: desc, width: w, height: h, scene, platform: currentPlatform },
      { button: $("btn-poster-ai") }
    );
    if (job.stopped) { $("status-text").textContent = t("job_stopped"); return; }
    $("poster-html").value = String(job.result).trim();
    posterTemplateDirty = true;
    updatePosterPreview();
    toast(lang === "zh-CN" ? "HTML 已生成，可直接编辑后导出" : "HTML generated; edit freely before export", "ok");
    window.Mozai && Mozai.celebrate();
  } catch (e) {
    toast(String(e), "err");
  } finally {
    $("status-text").textContent = t("ready");
  }
}

/* ------------------------------------------------------------- svg kit */
const SVG_SNIPPETS = {
  blink_btn: {
    label: () => (lang === "zh-CN" ? "闪烁引导按钮" : "Blinking CTA"),
    svg: `<section style="margin: 16px 0; text-align: center;"><svg xmlns="http://www.w3.org/2000/svg" width="240" height="56" viewBox="0 0 240 56" style="display: inline-block;"><rect x="2" y="2" width="236" height="52" rx="26" fill="#2F6CEA"><animate attributeName="opacity" values="1;0.45;1" dur="1.1s" begin="touchstart; click" repeatCount="2"/></rect><text x="120" y="35" text-anchor="middle" font-size="20" font-weight="600" fill="#FFFFFF">点击查看</text></svg></section>`,
  },
  draw_border: {
    label: () => (lang === "zh-CN" ? "描边绘制卡" : "Stroke-draw card"),
    svg: `<section style="margin: 16px 0;"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 677 140" style="width: 100%; display: block;"><rect x="2" y="2" width="673" height="136" rx="14" fill="#F7F8FA" stroke="#2F6CEA" stroke-width="3" stroke-dasharray="2000" stroke-dashoffset="0"><animate attributeName="stroke-dashoffset" values="2000;0" dur="1.4s" begin="touchstart; click" fill="freeze"/></rect><text x="30" y="62" font-size="24" font-weight="600" fill="#1F2328">点击绘制边框</text><text x="30" y="100" font-size="17" fill="#57606A">微信内点击卡片即可看到描边动画</text></svg></section>`,
  },
  fade_in: {
    label: () => (lang === "zh-CN" ? "渐显标语" : "Fade-in slogan"),
    svg: `<section style="margin: 16px 0; text-align: center;"><svg xmlns="http://www.w3.org/2000/svg" width="640" height="120" viewBox="0 0 640 120" style="width: 100%; max-width: 640px;"><text x="320" y="70" text-anchor="middle" font-size="30" font-weight="700" fill="#1F2328" opacity="1">写作是把噪音调成静音<animate attributeName="opacity" values="0;1" dur="1.2s" begin="touchstart; click" fill="freeze"/></text></svg></section>`,
  },
};

/* -------------------------------------------------------- image import */
const IMG_EXT = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"];
async function importDroppedImages(paths) {
  const imgs = paths.filter((p) => IMG_EXT.some((x) => p.toLowerCase().endsWith(x)));
  for (const p of imgs) {
    try {
      const asset = await invoke("import_image_from_path", { path: p });
      insertAtCursor(`\n![图片](${asset})\n`);
    } catch (e) {
      toast(String(e), "err");
    }
  }
  if (imgs.length > 0) {
    toast(t("img_imported", imgs.length), "ok");
    return true;
  }
  return false;
}

/* ----------------------------------------------------------------- init */
function autoGrow(el) {
  el.style.height = "auto";
  el.style.height = Math.min(el.scrollHeight, 120) + "px";
}

/* --------------------------------------------------------- fit studio */
let fitSourcePath = null;
let fitImage = null;

function fitDraw() {
  if (!fitImage) return;
  const [w, h] = $("fit-preset").value.split("x").map((x) => parseInt(x));
  const mode = $("fit-mode").value;
  const scale = parseInt($("fit-scale").value) || 1;
  const canvas = $("fit-canvas");
  const box = canvas.parentElement.clientWidth - 28 || 320;
  const dispScale = Math.min(1, box / w);
  canvas.style.width = w * dispScale + "px";
  canvas.style.height = h * dispScale + "px";
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  ctx.clearRect(0, 0, w, h);
  if (mode === "contain") {
    ctx.fillStyle = $("fit-bg").value || "#FFFFFF";
    ctx.fillRect(0, 0, w, h);
  }
  let dw, dh;
  if (mode === "cover") {
    const sc = Math.max(w / fitImage.width, h / fitImage.height);
    dw = fitImage.width * sc;
    dh = fitImage.height * sc;
  } else {
    const sc = Math.min(w / fitImage.width, h / fitImage.height);
    dw = fitImage.width * sc;
    dh = fitImage.height * sc;
  }
  ctx.drawImage(fitImage, (w - dw) / 2, (h - dh) / 2, dw, dh);
  $("fit-meta").textContent = `${w} × ${h} px · ${scale}x · ${mode === "cover" ? (lang === "zh-CN" ? "裁切填满" : "cover") : (lang === "zh-CN" ? "完整置入" : "contain")}`;
}
async function openFitStudio(path) {
  fitSourcePath = path;
  openModal("modal-fit");
  const uri = await invoke("asset_data_uri", { path });
  fitImage = await new Promise((res, rej) => {
    const im = new Image();
    im.onload = () => res(im);
    im.onerror = rej;
    im.src = uri;
  });
  fitDraw();
}
async function fitExport() {
  if (!fitImage || !invoke) return;
  const [w, h] = $("fit-preset").value.split("x").map((x) => parseInt(x));
  const scale = parseInt($("fit-scale").value) || 1;
  const mode = $("fit-mode").value;
  const canvas = document.createElement("canvas");
  canvas.width = w * scale;
  canvas.height = h * scale;
  const ctx = canvas.getContext("2d");
  ctx.scale(scale, scale);
  if (mode === "contain") {
    ctx.fillStyle = $("fit-bg").value || "#FFFFFF";
    ctx.fillRect(0, 0, w, h);
  }
  let dw, dh;
  if (mode === "cover") {
    const sc = Math.max(w / fitImage.width, h / fitImage.height);
    dw = fitImage.width * sc;
    dh = fitImage.height * sc;
  } else {
    const sc = Math.min(w / fitImage.width, h / fitImage.height);
    dw = fitImage.width * sc;
    dh = fitImage.height * sc;
  }
  ctx.drawImage(fitImage, (w - dw) / 2, (h - dh) / 2, dw, dh);
  const b64 = canvas.toDataURL("image/png").split(",")[1];
  const name = `fit-${w}x${h}-${Date.now()}.png`;
  const out = await invoke("import_image_bytes", { filename: name, base64Data: b64 });
  insertAtCursor(`
![配图](${out})
`);
  closeModal("modal-fit");
  toast(t("poster_inserted"), "ok");
  window.Mozai && Mozai.celebrate();
}

/* Device frames */
const DEVICE_OUTER = { ios: [413, 872], android: [432, 935] };
function applyDevice() {
  const kind = localStorage.getItem("wxwright-device") || "ios";
  document.querySelectorAll(".seg-btn").forEach((b) => {
    b.classList.toggle("active", b.dataset.device === kind);
  });
  const device = $("device");
  device.classList.remove("ios", "android");
  device.classList.add(kind);
  fitDevice();
}
function fitDevice() {
  const stage = $("device-stage");
  const pane = stage.closest(".pane-preview");
  if (!pane) return;
  const kind = localStorage.getItem("wxwright-device") || "ios";
  const [w, h] = DEVICE_OUTER[kind];
  const availH = pane.clientHeight - 86;
  const availW = pane.clientWidth - 32;
  const scale = Math.max(0.3, Math.min(1, availH / h, availW / w));
  const scaler = $("device-scaler");
  scaler.style.width = w * scale + "px";
  scaler.style.height = h * scale + "px";
  const dev = $("device");
  dev.style.transform = "scale(" + scale + ")";
  dev.style.transformOrigin = "top left";
}
function toggleDark() {
  darkPreview = !darkPreview;
  $("device").classList.toggle("dark", darkPreview);
  $("btn-device-dark").querySelector("use").setAttribute("href", darkPreview ? "#i-sun" : "#i-moon");
  localStorage.setItem("wxwright-dark", darkPreview ? "1" : "0");
  if (darkPreview) toast(t("dark_hint"));
}

/* ---------------------------------------------------------- attachments */
let pendingAttachments = []; // {kind:"text"|"image", name, content|dataUri}
function renderAttachRow() {
  const row = $("attach-row");
  row.innerHTML = "";
  row.hidden = pendingAttachments.length === 0;
  for (const a of pendingAttachments) {
    const chip = document.createElement("span");
    chip.className = "attach-chip";
    const icon = a.kind === "image" ? "i-image" : "i-clip";
    chip.innerHTML = `<svg class="icon icon-sm"><use href="#${icon}"/></svg><span class="a-name">${escapeHtml(a.name)}</span><button type="button" title="remove"><svg class="icon icon-sm"><use href="#i-x"/></svg></button>`;
    chip.querySelector("button").addEventListener("click", () => {
      pendingAttachments = pendingAttachments.filter((x) => x !== a);
      renderAttachRow();
    });
    row.appendChild(chip);
  }
}
async function addAttachment() {
  if (!invoke) { toast(t("demo_mode"), "err"); return; }
  if (pendingAttachments.length >= 4) { toast(lang === "zh-CN" ? "最多 4 个附件" : "Max 4 attachments", "err"); return; }
  try {
    const { open } = window.__TAURI__.dialog;
    const path = await open({
      multiple: false,
      filters: [
        { name: lang === "zh-CN" ? "文档与图片" : "Documents & images", extensions: ["md", "markdown", "txt", "html", "htm", "pdf", "docx", "csv", "json", "xml", "png", "jpg", "jpeg", "webp"] },
      ],
    });
    if (!path) return;
    const p = String(path);
    const lower = p.toLowerCase();
    const name = p.split(/[\\/]/).pop();
    const IMG = ["png", "jpg", "jpeg", "webp"];
    const ext = lower.includes(".") ? lower.split(".").pop() : "";
    if (IMG.includes(ext)) {
      // image -> vision attachment (base64 data URI, sent as image_url)
      const b64 = await invoke("read_binary_file", { path: p });
      const dataUri = `data:image/${ext === "jpg" ? "jpeg" : ext};base64,${b64}`;
      pendingAttachments.push({ kind: "image", name, dataUri });
    } else {
      // document -> backend text extraction (pdf/docx/html/txt via extract)
      const doc = await invoke("extract_document_text", { path: p });
      pendingAttachments.push({ kind: "text", name, content: doc.text, truncated: doc.truncated });
    }
    renderAttachRow();
  } catch (e) {
    toast(String(e), "err");
  }
}

/* ------------------------------------------------------ brand marks */
/* Hand-drawn inline marks in official brand colors (zero external requests).
   A provider can override with a custom `logo` data URI in settings. */
const BRAND_MARKS = {
  openai: {
    match: ["openai"],
    bg: "#0D0D0D",
    mark: '<g fill="#FFFFFF"><path d="M12 3.4c2.6 0 4.1 2.2 3.5 4.5l-.3 1.1a4.9 4.9 0 0 1 3.5 6.8 4.9 4.9 0 0 1-6.9 6.9A4.9 4.9 0 0 1 5.3 19 4.9 4.9 0 0 1 5 12.4a4.9 4.9 0 0 1 3.2-6.9C8.7 3.6 10.2 3.4 12 3.4Z" opacity=".95"/></g>',
  },
  anthropic: {
    match: ["anthropic", "claude"],
    bg: "#CC785C",
    mark: '<path fill="#FFFFFF" d="M12 3.6 14 10l6.4 2L14 14l-2 6.4L10 14l-6.4-2L10 10Z"/>',
  },
  gemini: {
    match: ["gemini", "google"],
    bg: "#1C69FF",
    mark: '<path fill="#FFFFFF" d="M12 2.6c.7 5.2 4.2 8.7 9.4 9.4-5.2.7-8.7 4.2-9.4 9.4-.7-5.2-4.2-8.7-9.4-9.4 5.2-.7 8.7-4.2 9.4-9.4Z"/>',
  },
  deepseek: {
    match: ["deepseek"],
    bg: "#4D6BFE",
    mark: '<path fill="#FFFFFF" d="M20.5 8.2c-2.7-3.2-7.9-3.6-11-.9-2.7 2.3-3.3 6-1.7 8.9-.9.4-1.9 1.1-2.4 2 1.4.1 2.8-.3 3.8-1 3.4 1.8 7.8 1.1 10.2-1.7 1.6-1.9 1.9-4.5 1.1-7.3ZM15 12.4a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"/>',
  },
  qwen: {
    match: ["qwen", "tongyi", "dashscope"],
    bg: "#6236FF",
    mark: '<g fill="none" stroke="#FFFFFF" stroke-width="2"><circle cx="12" cy="12" r="6.4"/></g><path fill="#FFFFFF" d="M11 11.2h2v6h-2z"/><circle cx="12" cy="12" r="2.2" fill="#FFFFFF"/>',
  },
  kimi: {
    match: ["kimi", "moonshot"],
    bg: "#16191E",
    mark: '<path fill="#FFFFFF" d="M9 5v14h2.2v-5.4L15.4 19H18l-4.6-6.2L17.8 5h-2.7l-3.9 6V5Z"/>',
  },
  zhipu: {
    match: ["zhipu", "glm", "chatglm"],
    bg: "#3859FF",
    mark: '<path fill="#FFFFFF" d="M6 6h12v2.6h-8.6v3.2H16v2.4H9.4v3.2H18V20H6Z"/>',
  },
  ollama: {
    match: ["ollama"],
    bg: "#0F0F0F",
    mark: '<path fill="#FFFFFF" d="M12 3.6c3.9 0 6.8 2.9 6.8 6.7 0 2.4-1.2 4.4-3 5.6V20h-2.2v-2.2h-3.2V20H8.2v-4.1c-1.8-1.2-3-3.2-3-5.6 0-3.8 2.9-6.7 6.8-6.7Zm-2.4 6.2a1.3 1.3 0 1 0 0 2.6 1.3 1.3 0 0 0 0-2.6Zm4.8 0a1.3 1.3 0 1 0 0 2.6 1.3 1.3 0 0 0 0-2.6Z"/>',
  },
  lmstudio: {
    match: ["lmstudio", "lm-studio", "localhost:1234"],
    bg: "#4B87FF",
    mark: '<path fill="#FFFFFF" d="M5 5h3.2v9.6H19V18H5Z"/>',
  },
  agnes: {
    match: ["agnes"],
    bg: "#111827",
    mark: '<path fill="#FFFFFF" d="M6 19 11 5h2l5 14h-2.4l-1.2-3.4H9.6L8.4 19Zm4.3-5.6h3.4L12 8.4Z"/>',
  },
  openrouter: {
    match: ["openrouter"],
    bg: "#6467F2",
    mark: '<path fill="#FFFFFF" d="M5 5h9a5 5 0 0 1 1.6 9.7L19 19h-2.8l-3-4H7.8V19H5.8L5 5Zm2.8 2.2v3.6H14a1.8 1.8 0 0 0 0-3.6Z"/>',
  },
};

/* vendor presets: one-click provider forms (official logos via BRAND_LOGOS) */
const VENDOR_PRESETS = {
  openai: { name: "OpenAI", base: "https://api.openai.com", model: "gpt-4o", brand: "openai" },
  deepseek: { name: "DeepSeek", base: "https://api.deepseek.com", model: "deepseek-chat", brand: "deepseek" },
  kimi: { name: "Moonshot Kimi", base: "https://api.moonshot.cn", model: "kimi-latest", brand: "moonshotai" },
  qwen: { name: "通义千问", base: "https://dashscope.aliyuncs.com/compatible-mode", model: "qwen-plus", brand: "qwen" },
  zhipu: { name: "智谱 GLM", base: "https://open.bigmodel.cn", model: "glm-4-plus", brand: "zhipu" },
  gemini: { name: "Google Gemini", base: "https://generativelanguage.googleapis.com", model: "gemini-2.0-flash", brand: "googlegemini" },
  xai: { name: "xAI Grok", base: "https://api.x.ai", model: "grok-3", brand: "xai" },
  mistral: { name: "Mistral", base: "https://api.mistral.ai", model: "mistral-large-latest", brand: "mistralai" },
  openrouter: { name: "OpenRouter", base: "https://openrouter.ai/api", model: "openai/gpt-4o", brand: "openrouter" },
  together: { name: "Together", base: "https://api.together.xyz", model: "meta-llama/Llama-3-70B-Instruct-Turbo", brand: "meta" },
  ollama: { name: "Ollama 本地", base: "http://localhost:11434", model: "qwen2.5:14b", brand: "ollama" },
  lmstudio: { name: "LM Studio 本地", base: "http://localhost:1234", model: "local-model", brand: "lmstudio" },
  agnes: { name: "Agnes AI", base: "https://apihub.agnes-ai.com", model: "agnes-3.0-flash", brand: "agnes" },
};

function brandFor(provider) {
  // 1. official logo library (brand-logos.js, Simple Icons CC0) — explicit
  //    brand key first, then keyword match on name/base_url/model
  const LIB = window.BRAND_LOGOS || {};
  if (provider && provider.brand && LIB[provider.brand]) {
    return { key: provider.brand, ...LIB[provider.brand] };
  }
  const hay = [provider && provider.name, provider && provider.base_url, provider && provider.model]
    .filter(Boolean).join(" ").toLowerCase();
  for (const [key, entry] of Object.entries(LIB)) {
    if ((entry.match || [key]).some((m) => hay.includes(m))) {
      return { key, ...entry };
    }
  }
  // 2. legacy hand-drawn marks (agnes and any future custom marks)
  for (const [key, brand] of Object.entries(BRAND_MARKS)) {
    if (brand.match.some((m) => hay.includes(m))) return { key, ...brand };
  }
  return null;
}

function brandTileHTML(provider, size) {
  size = size || 18;
  const radius = Math.max(3, Math.round(size * 0.28));
  const font = Math.max(8, Math.round(size * 0.5));
  // 1. custom logo wins
  if (provider && provider.logo) {
    return `<span class="brand-tile" style="width:${size}px;height:${size}px;background:transparent;border-radius:${radius}px;">
      <img src="${escapeHtml(provider.logo)}" style="width:100%;height:100%;object-fit:contain;" alt="" />
    </span>`;
  }
  const brand = brandFor(provider || {});
  const letter = ((provider && provider.name) || "?").trim().charAt(0).toUpperCase();
  if (!brand) {
    return `<span class="brand-tile" style="width:${size}px;height:${size}px;background:var(--border-strong);border-radius:${radius}px;font-size:${font}px">${escapeHtml(letter)}</span>`;
  }
  // official data-URI logo (e.g. Agnes): white tile, transparent vendor PNG
  if (brand.dataUri) {
    return `<span class="brand-tile" style="width:${size}px;height:${size}px;background:#FFFFFF;border:1px solid var(--border);border-radius:${radius}px">
      <img src="${escapeHtml(brand.dataUri)}" style="width:78%;height:78%;object-fit:contain;" alt="" />
    </span>`;
  }
  // official SVG path (Simple Icons): brand-color tile + mark
  if (brand.path) {
    const fg = brand.dark ? "#1F2328" : "#FFFFFF";
    return `<span class="brand-tile" style="width:${size}px;height:${size}px;background:${brand.color};border-radius:${radius}px">
      <svg width="${Math.round(size * 0.74)}" height="${Math.round(size * 0.74)}" viewBox="0 0 24 24" aria-hidden="true"><path fill="${fg}" d="${brand.path}"/></svg>
    </span>`;
  }
  // legacy hand-drawn fallback
  return `<span class="brand-tile" style="width:${size}px;height:${size}px;background:${brand.bg || "var(--border-strong)"};border-radius:${radius}px">
    <svg width="${Math.round(size * 0.72)}" height="${Math.round(size * 0.72)}" viewBox="0 0 24 24" aria-hidden="true">${brand.mark || ""}</svg>
  </span>`;
}

/* ------------------------------------------------------ model quick-select */
let modelMenuState = { providers: [], active: null };
async function refreshModelSelect() {
  const btn = $("ai-model-select");
  if (!invoke) {
    // browser demo: sample providers so the brand marks/menu are visible
    modelMenuState = {
      providers: [
        { id: "openai", name: "OpenAI", model: "gpt-4o", logo: null },
        { id: "claude", name: "Anthropic", model: "claude-sonnet-4", logo: null },
        { id: "deepseek", name: "DeepSeek", model: "deepseek-chat", logo: null },
        { id: "kimi", name: "Kimi", model: "kimi-latest", logo: null },
        { id: "ollama", name: "Ollama 本地", model: "qwen2.5:14b", logo: null },
      ],
      active: "openai",
    };
    const active = modelMenuState.providers.find((p) => p.id === modelMenuState.active);
    btn.innerHTML = `${brandTileHTML(active, 15)}<span>${escapeHtml(active.name)}</span><svg class="m-chev" viewBox="0 0 24 24"><path d="m6 9 6 6 6-6" fill="none" stroke="currentColor" stroke-width="2"/></svg>`;
    return;
  }
  try {
    const st = await invoke("ai_settings");
    modelMenuState = { providers: st.providers, active: st.active };
    const active = st.providers.find((p) => p.id === st.active);
    // mainstream: closed state shows brand mark + provider (vendor) name only
    btn.innerHTML = `${brandTileHTML(active, 15)}<span>${escapeHtml(active ? active.name : (lang === "zh-CN" ? "未配置" : "no model"))}</span><svg class="m-chev" viewBox="0 0 24 24"><path d="m6 9 6 6 6-6" fill="none" stroke="currentColor" stroke-width="2"/></svg>`;
    aiProviderChip(st);
  } catch (e) { /* settings unreadable: leave as-is */ }
}

function renderModelMenu() {
  const menu = $("ai-model-menu");
  if (!menu.children.length || true) {
    menu.innerHTML = modelMenuState.providers
      .map(
        (pr) => `<div class="mm-item${pr.id === modelMenuState.active ? " active" : ""}" data-id="${escapeHtml(pr.id)}">
          ${brandTileHTML(pr, 20)}
          <div class="mm-main">
            <div class="mm-name">${escapeHtml(pr.name)}</div>
            <div class="mm-model">${escapeHtml(pr.model)}</div>
          </div>
          <svg class="mm-check" viewBox="0 0 24 24"><path d="M20 6 9 17l-5-5" fill="none" stroke="currentColor" stroke-width="2"/></svg>
        </div>`
      )
      .join("");
    menu.querySelectorAll(".mm-item").forEach((row) => {
      row.addEventListener("click", async () => {
        if (!invoke) return;
        try {
          const st = await invoke("ai_set_active", { id: row.dataset.id });
          modelMenuState = { providers: st.providers, active: st.active };
          aiProviderChip(st);
          $("ai-model-menu").hidden = true;
          $("ai-model-select").classList.remove("open");
          await refreshModelSelect();
          toast(lang === "zh-CN" ? "已切换模型" : "Model switched", "ok");
        } catch (err) {
          toast(String(err), "err");
        }
      });
    });
  }
}

/* ------------------------------------------------------ comfy image src */
let comfyImgSrc = "comfy"; // comfy | cloud
function setComfyImgSrc(src) {
  comfyImgSrc = src;
  $("imgsrc-comfy").classList.toggle("active", src === "comfy");
  $("imgsrc-cloud").classList.toggle("active", src === "cloud");
  $("comfy-cloud-block").hidden = src !== "cloud";
  $("comfy-i2i-row").hidden = src !== "comfy" || comfyMode !== "i2i";
}

/* --------------------------------------------------------- svg kit */
let svgKitCustom = [];
try { svgKitCustom = JSON.parse(localStorage.getItem("wxwright-svgkit-custom") || "[]"); } catch (e) { svgKitCustom = []; }
function saveCustomSvg() {
  try { localStorage.setItem("wxwright-svgkit-custom", JSON.stringify(svgKitCustom.slice(0, 30))); } catch (e) {}
}

const SVG_KIT = {
  blink: {
    name: () => (lang === "zh-CN" ? "闪烁引导按钮" : "Blinking CTA"),
    desc: () => (lang === "zh-CN" ? "读者点击/触摸时按钮闪烁两次，常用于引导关注与点击。" : "Blinks twice on tap; use for CTAs."),
    params: { text: "点击查看", color: "#2F6CEA", textColor: "#FFFFFF" },
    fields: [
      ["text", "按钮文字"],
      ["color", "背景色（色值）"],
      ["textColor", "文字颜色"],
    ],
    build: (p) => `<section style="margin: 16px 0; text-align: center;"><svg xmlns="http://www.w3.org/2000/svg" width="240" height="56" viewBox="0 0 240 56" style="display: inline-block;"><rect x="2" y="2" width="236" height="52" rx="26" fill="${p.color}"><animate attributeName="opacity" values="1;0.45;1" dur="1.1s" begin="touchstart; click" repeatCount="2"/></rect><text x="120" y="35" text-anchor="middle" font-size="20" font-weight="600" fill="${p.textColor}">${escapeHtml(p.text)}</text></svg></section>`,
  },
  fade: {
    name: () => (lang === "zh-CN" ? "渐显金句" : "Fade-in slogan"),
    desc: () => (lang === "zh-CN" ? "点击后金句渐显，适合做章节金句与强调。" : "Fades the line in on tap."),
    params: { text: "写作是把噪音调成静音。", color: "#1F2328" },
    fields: [
      ["text", "金句文字"],
      ["color", "文字颜色"],
    ],
    build: (p) => `<section style="margin: 16px 0; text-align: center;"><svg xmlns="http://www.w3.org/2000/svg" width="640" height="120" viewBox="0 0 640 120" style="width: 100%; max-width: 640px;"><text x="320" y="70" text-anchor="middle" font-size="30" font-weight="700" fill="${p.color}" opacity="1">${escapeHtml(p.text)}<animate attributeName="opacity" values="0;1" dur="1.2s" begin="touchstart; click" fill="freeze"/></text></svg></section>`,
  },
  expand: {
    name: () => (lang === "zh-CN" ? "点击展开卡" : "Tap-to-reveal card"),
    desc: () => (lang === "zh-CN" ? "点击卡片后描边逐渐画出，暗示有隐藏内容。" : "Stroke draws itself on tap."),
    params: { title: "点我看详情", body: "这段内容在微信里点击卡片即可看到描边动画。", color: "#2F6CEA" },
    fields: [
      ["title", "标题"],
      ["body", "正文"],
      ["color", "描边颜色"],
    ],
    build: (p) => `<section style="margin: 16px 0;"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 677 140" style="width: 100%; display: block;"><rect x="2" y="2" width="673" height="136" rx="14" fill="#F7F8FA" stroke="${p.color}" stroke-width="3" stroke-dasharray="2000" stroke-dashoffset="0"><animate attributeName="stroke-dashoffset" values="2000;0" dur="1.4s" begin="touchstart; click" fill="freeze"/></rect><text x="30" y="62" font-size="24" font-weight="600" fill="#1F2328">${escapeHtml(p.title)}</text><text x="30" y="100" font-size="17" fill="#57606A">${escapeHtml(p.body)}</text></svg></section>`,
  },
  swap: {
    name: () => (lang === "zh-CN" ? "点击切换 (A/B)" : "A/B tap swap"),
    desc: () => (lang === "zh-CN" ? "点击在两句话之间切换，适合做对比、投票感或反转梗。" : "Swap between two lines on tap."),
    params: { textA: "写公众号最难的是什么？", textB: "是点击之后你看到了答案。", color: "#2F6CEA" },
    fields: [
      ["textA", "第一句（A）"],
      ["textB", "第二句（B）"],
      ["color", "强调色"],
    ],
    build: (p) => `<section style="margin: 16px 0; text-align: center;"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 677 150" style="width: 100%; max-width: 677px;"><rect x="1" y="1" width="675" height="148" rx="12" fill="${p.color}" opacity="0.06"/><text x="338" y="70" text-anchor="middle" font-size="24" font-weight="600" fill="${p.color}">${escapeHtml(p.textA)}<animate attributeName="opacity" values="1;0" dur="0.4s" begin="touchstart; click" fill="freeze"/></text><text x="338" y="70" text-anchor="middle" font-size="24" font-weight="600" fill="#1F2328" opacity="0">${escapeHtml(p.textB)}<animate attributeName="opacity" values="0;1" dur="0.4s" begin="touchstart; click" fill="freeze"/></text><text x="338" y="126" text-anchor="middle" font-size="14" fill="#8B949E">点击切换</text></svg></section>`,
  },
  countdown: {
    name: () => (lang === "zh-CN" ? "点击点亮进度条" : "Tap-to-fill bar"),
    desc: () => (lang === "zh-CN" ? "点击后进度条从 0 涨到 100%，适合进度、完成度、投票结果。" : "Bar fills up on tap."),
    params: { label: "本期阅读完成率", color: "#2F6CEA" },
    fields: [
      ["label", "标签文字"],
      ["color", "进度条颜色"],
    ],
    build: (p) => `<section style="margin: 16px 0;"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 677 110" style="width: 100%; display: block;"><text x="2" y="34" font-size="18" font-weight="600" fill="#1F2328">${escapeHtml(p.label)}</text><rect x="2" y="54" width="673" height="18" rx="9" fill="#E4E7EC"/><rect x="2" y="54" width="673" height="18" rx="9" fill="${p.color}"><animate attributeName="width" values="0;640" dur="1.6s" begin="touchstart; click" fill="freeze"/></rect><text x="2" y="98" font-size="14" fill="#8B949E">点击查看</text></svg></section>`,
  },
  picfade: {
    name: () => (lang === "zh-CN" ? "图片渐显卡（可传图）" : "Image fade card"),
    desc: () => (lang === "zh-CN" ? "上传一张图，点击后在公众号里渐显并带文案条——图文互动的基础组件。" : "Upload an image; it fades in with a caption on tap."),
    params: { image: "", text: "点击查看图片", color: "#2F6CEA" },
    fields: [
      ["image", "上传图片（自动内嵌 data URI）"],
      ["text", "图片下方文案"],
      ["color", "文案颜色"],
    ],
    build: (p) => {
      // unique clip id per instance (two picfades in one article must not collide)
      const uid = "wc" + Math.abs([...(p.text + p.color)].reduce((a, c) => a + c.charCodeAt(0), 7)) + Date.now().toString(36).slice(-4);
      const imgTag = p.image
        ? `<image href="${p.image}" xlink:href="${p.image}" x="8" y="8" width="661" height="360" preserveAspectRatio="xMidYMid slice" clip-path="url(#${uid})"><animate attributeName="opacity" values="0.25;1" dur="0.9s" begin="touchstart; click" fill="freeze"/></image>`
        : `<rect x="8" y="8" width="661" height="360" fill="#EFF4FE"/><text x="338" y="200" text-anchor="middle" font-size="20" fill="#8B949E">${escapeHtml(lang === "zh-CN" ? "先上传一张图片" : "Upload an image first")}</text>`;
      return `<section style="margin: 16px 0;"><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 677 430" style="width: 100%; display: block;"><defs><clipPath id="${uid}"><rect x="8" y="8" width="661" height="360" rx="12"/></clipPath></defs>${imgTag}<text x="8" y="404" font-size="18" font-weight="600" fill="${p.color}">${escapeHtml(p.text)}</text></svg></section>`;
    },
  },
};

let svgKitKey = "blink";
let svgKitParams = {};

function currentSvgMarkup() {
  if (svgKitKey.startsWith("c")) {
    const idx = parseInt(svgKitKey.slice(1));
    return svgKitCustom[idx] ? svgKitCustom[idx].svg : "";
  }
  const kit = SVG_KIT[svgKitKey];
  return kit ? kit.build(svgKitParams) : "";
}

async function svgKitAiGenerate() {
  if (jobBusy.svg) return;
  const desc = $("svgkit-desc").value.trim();
  if (!desc) { toast(lang === "zh-CN" ? "请先描述想要的互动效果" : "Describe the effect first", "err"); return; }
  if (!(await hasProvider())) { toast(lang === "zh-CN" ? "请先在设置中配置 AI Provider" : "Configure an AI provider first", "err"); return; }
  jobBusy.svg = true;
  $("svgkit-ai-status").textContent = "";
  try {
    const job = await AIJobs.run("svg", { description: desc }, { button: $("svgkit-ai-generate") });
    if (job.stopped) { $("svgkit-ai-status").textContent = t("job_stopped"); return; }
    const svg = job.result;
    svgKitCustom.unshift({ id: "c" + Date.now(), name: desc.slice(0, 14), svg });
    saveCustomSvg();
    svgKitKey = "c0";
    svgKitParams = {};
    renderSvgKit();
    toast(lang === "zh-CN" ? "组件已生成，预览后即可插入" : "Component generated", "ok");
    if (window.Mozai) Mozai.celebrate();
  } catch (e) {
    $("svgkit-ai-status").textContent = String(e);
    toast(String(e), "err");
  } finally {
    delete jobBusy.svg;
  }
}

function renderSvgKit() {
  const list = $("svgkit-list");
  list.innerHTML = "";
  const mkItem = (key, name, desc, badge, delIdx) => {
    const item = document.createElement("div");
    item.className = "svgkit-item" + (key === svgKitKey ? " active" : "");
    item.innerHTML = `<span class="k-name">${escapeHtml(name)}${badge ? `<span class="k-badge">${badge}</span>` : ""}</span><span class="k-desc">${escapeHtml(desc)}</span>${delIdx !== null ? `<button type="button" class="k-del" data-del="${delIdx}">删除</button>` : ""}`;
    item.addEventListener("click", (e) => {
      if (e.target.closest(".k-del")) return;
      svgKitKey = key;
      svgKitParams = key.startsWith("c") ? {} : { ...SVG_KIT[key].params };
      renderSvgKit();
    });
    list.appendChild(item);
  };
  for (const key of Object.keys(SVG_KIT)) {
    mkItem(key, SVG_KIT[key].name(), SVG_KIT[key].desc(), null, null);
  }
  svgKitCustom.forEach((c, i) => {
    mkItem("c" + i, c.name, lang === "zh-CN" ? "AI 生成的自定义组件" : "AI-generated custom", "AI", i);
  });
  list.querySelectorAll("[data-del]").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      svgKitCustom.splice(parseInt(btn.dataset.del), 1);
      saveCustomSvg();
      if (svgKitKey.startsWith("c")) svgKitKey = "blink";
      renderSvgKit();
    });
  });

  const isCustom = svgKitKey.startsWith("c");
  let kit;
  if (isCustom) {
    const idx = parseInt(svgKitKey.slice(1));
    kit = { fields: [] };
  } else {
    kit = SVG_KIT[svgKitKey] || SVG_KIT.blink;
    svgKitParams = { ...kit.params, ...svgKitParams };
  }
  const descView = $("svgkit-desc-view");
  if (descView) {
    descView.textContent = isCustom
      ? (lang === "zh-CN" ? "AI 生成组件为固定片段，可在插入后于编辑器中微调文字。" : "AI components are fixed snippets; fine-tune text in the editor after inserting.")
      : kit.desc();
  }
  const form = $("svgkit-params");
  form.innerHTML = "";
  if (isCustom) {
    form.innerHTML = `<div class="section-hint" style="margin:0;">${lang === "zh-CN" ? "AI 生成组件为固定片段，可在插入后于编辑器中微调文字。" : "AI components are fixed snippets; fine-tune text in the editor after inserting."}</div>`;
  } else {
    for (const [key, label] of kit.fields) {
      const wrap = document.createElement("label");
      wrap.className = key.includes("Color") || key === "color" ? "" : "span2";
      if (key === "image") {
        wrap.innerHTML = `<span>${label}</span>
          <div class="form-actions" style="margin-top: 0;">
            <input data-param="${key}" readonly placeholder="${lang === "zh-CN" ? "尚未上传" : "no image yet"}" style="flex: 1;" />
            <button type="button" class="btn btn-ghost" data-imgpick="${key}">${lang === "zh-CN" ? "上传图片" : "Upload"}</button>
          </div>`;
        form.appendChild(wrap);
      } else {
        const isColor = key === "color" || key.toLowerCase().endsWith("color");
        if (isColor) {
          const val = String(svgKitParams[key] ?? "");
          const hex = /^#[0-9a-fA-F]{6}$/.test(val) ? val : "#2F6CEA";
          wrap.innerHTML = `<span>${label}</span>
            <div class="form-actions" style="margin-top: 0;">
              <input data-param="${key}" value="${escapeHtml(val)}" style="flex: 1;" />
              <input type="color" data-colorpick="${key}" value="${hex}" class="color-pick" title="${lang === "zh-CN" ? "取色器" : "Color picker"}" />
            </div>`;
        } else {
          wrap.innerHTML = `<span>${label}</span><input data-param="${key}" value="${escapeHtml(svgKitParams[key] ?? "")}" />`;
        }
        form.appendChild(wrap);
      }
    }
    form.querySelectorAll("input[data-param]").forEach((inp) => {
      inp.addEventListener("input", () => {
        svgKitParams[inp.dataset.param] = inp.value;
        renderSvgKitPreview();
      });
    });
    form.querySelectorAll("input[data-colorpick]").forEach((pick) => {
      pick.addEventListener("input", () => {
        svgKitParams[pick.dataset.colorpick] = pick.value;
        const twin = form.querySelector(`input[data-param="${pick.dataset.colorpick}"]`);
        if (twin) twin.value = pick.value;
        renderSvgKitPreview();
      });
    });
    form.querySelectorAll("button[data-imgpick]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const inpEl = document.createElement("input");
        inpEl.type = "file";
        inpEl.accept = "image/png,image/jpeg,image/webp";
        inpEl.onchange = () => {
          const f = inpEl.files && inpEl.files[0];
          if (!f) return;
          const r = new FileReader();
          r.onload = () => {
            svgKitParams[btn.dataset.imgpick] = String(r.result);
            renderSvgKit();
          };
          r.readAsDataURL(f);
        };
        inpEl.click();
      });
    });
  }
  renderSvgKitPreview();
}

function renderSvgKitPreview() {
  const html = `<!doctype html><html><head><meta charset="utf-8"><style>body{margin:0;padding:18px;display:flex;align-items:center;justify-content:center;background:#fff;}</style></head><body>${currentSvgMarkup()}</body></html>`;
  $("svgkit-preview").innerHTML = `<iframe srcdoc="${escapeHtml(html)}"></iframe>`;
}

/* ------------------------------------------------------ assets panel */
let assetsSelectResolve = null;
let pendingAssetsCache = null;

async function openAssetsPanel(opts = {}) {
  opts = opts || {};
  openModal("modal-assets");
  const grid = $("assets-grid");
  $("assets-open-folder").onclick = () => {
    if (window.__TAURI__ && window.__TAURI__.opener) {
      invoke("library_dir").then((d) => {
        const dir = d.replace(/articles$/, "assets");
        window.__TAURI__.opener.openPath(dir).catch(() => {
          window.__TAURI__.opener.revealItemInDir(dir).catch(() => {});
        });
      });
    } else {
      toast(t("demo_mode"), "err");
    }
  };
  if (opts.selectMode) grid.dataset.selectMode = "1";
  else delete grid.dataset.selectMode;

  if (!invoke) {
    // Browser demo: placeholder cards so the grid design is verifiable.
    $("assets-path").textContent = "C:\\Users\\you\\Documents\\wxwright\\assets（演示）";
    $("assets-count").textContent = "3 张";
    const names = ["poster-1080x460.png", "t2i-8841-0.png", "paste-771.png"];
    grid.innerHTML = "";
    for (const n of names) {
      const item = document.createElement("div");
      item.className = "asset-item";
      item.innerHTML = `<div style="height:96px;display:flex;align-items:center;justify-content:center;color:var(--text-tertiary);background:var(--bg-subtle);">
          <svg class="icon" style="width:26px;height:26px;"><use href="#i-image"/></svg></div>
        <div class="a-meta">${escapeHtml(n)} · demo</div>
        <div class="a-tools">
          <button type="button" class="ins"><svg class="icon icon-sm"><use href="#i-plus"/></svg></button>
          <button type="button" class="fit"><svg class="icon icon-sm"><use href="#i-crop"/></svg></button>
          <button type="button" class="del"><svg class="icon icon-sm"><use href="#i-trash"/></svg></button>
        </div>`;
      // demo stubs still answer every button (honest boundary, not dead UI)
      item.querySelector(".ins").addEventListener("click", () => toast(t("demo_mode"), "err"));
      item.querySelector(".fit").addEventListener("click", () => toast(t("demo_mode"), "err"));
      item.querySelector(".del").addEventListener("click", () => toast(t("demo_mode"), "err"));
      grid.appendChild(item);
    }
    return;
  }

  const dir = await invoke("library_dir").then((d) => d.replace(/articles$/, "assets")).catch(() => "");
  $("assets-path").textContent = dir;
  const assets = await invoke("list_assets");
  $("assets-count").textContent = `${assets.length} ${lang === "zh-CN" ? "张" : "items"}`;
  if (assets.length === 0) {
    grid.innerHTML = `<div class="assets-empty">
      <svg class="icon"><use href="#i-image"/></svg>
      <div class="t">${lang === "zh-CN" ? "素材库还是空的" : "The asset library is empty"}</div>
      <div class="s">${lang === "zh-CN" ? "海报工坊导出的头图、ComfyUI 生成的图片、粘贴的截图、拖进窗口的图片，都会自动收进这里。" : "Exported posters, ComfyUI images, pasted screenshots and dropped images all land here."}</div>
      <button class="btn btn-ghost" id="assets-goto-poster">${lang === "zh-CN" ? "去海报工坊生成一张" : "Open Poster Studio"}</button>
    </div>`;
    const gotoBtn = grid.querySelector("#assets-goto-poster");
    if (gotoBtn) gotoBtn.addEventListener("click", () => { closeModal("modal-assets"); $("btn-poster").click(); });
    return;
  }

  grid.innerHTML = "";
  const shown = assets.slice(0, 60);
  for (const a of shown) {
    const item = document.createElement("div");
    item.className = "asset-item";
    item.dataset.path = a.path;
    item.innerHTML = `<img alt="${escapeHtml(a.name)}" />
      <div class="a-meta" title="${escapeHtml(a.path)}">${escapeHtml(a.name)} · ${(a.size / 1024).toFixed(0)}KB</div>
      <div class="a-tools">
        <button type="button" class="ins" title="${t("insert_article")}"><svg class="icon icon-sm"><use href="#i-plus"/></svg></button>
        <button type="button" class="fit" title="${lang === "zh-CN" ? "尺寸工坊" : "Fit to MP sizes"}"><svg class="icon icon-sm"><use href="#i-crop"/></svg></button>
        <button type="button" class="del" title="delete"><svg class="icon icon-sm"><use href="#i-trash"/></svg></button>
      </div>`;
    grid.appendChild(item);
    invoke("asset_thumb", { path: a.path, size: 320 }).then((uri) => {
      const im = item.querySelector("img");
      if (im) im.src = uri;
    }).catch(() => {
      invoke("asset_data_uri", { path: a.path }).then((uri) => {
        const im = item.querySelector("img");
        if (im) im.src = uri;
      }).catch(() => {});
    });
    item.addEventListener("click", async (e) => {
      if (e.target.closest(".a-tools")) return;
      if (grid.dataset.selectMode === "1") {
        item.classList.add("selected");
        const picked = a.path;
        if (assetsSelectResolve) { assetsSelectResolve(picked); assetsSelectResolve = null; }
        closeModal("modal-assets");
        return;
      }
      insertAtCursor(`\n![图片](${a.path})\n`);
      toast(t("poster_inserted"), "ok");
    });
    item.querySelector(".ins").addEventListener("click", () => {
      insertAtCursor(`\n![图片](${a.path})\n`);
      toast(t("poster_inserted"), "ok");
    });
    item.querySelector(".fit").addEventListener("click", () => openFitStudio(a.path));
    item.querySelector(".del").addEventListener("click", async (e) => {
      e.stopPropagation();
      await invoke("delete_asset", { path: a.path });
      toast(t("deleted_ok"), "ok");
      openAssetsPanel({ selectMode: grid.dataset.selectMode === "1" });
    });
  }
}

/* ------------------------------------------------------ comfy panel */
let comfyMode = "t2i";
function setComfyMode(mode) {
  comfyMode = mode;
  $("comfy-mode-t2i").style.background = mode === "t2i" ? "var(--accent)" : "";
  $("comfy-mode-t2i").style.color = mode === "t2i" ? "#fff" : "";
  $("comfy-mode-i2i").style.background = mode === "i2i" ? "var(--accent)" : "";
  $("comfy-mode-i2i").style.color = mode === "i2i" ? "#fff" : "";
  $("comfy-i2i-row").hidden = mode !== "i2i";
}
function renderComfyStatus(st) {
  const line = $("comfy-status-line");
  const chip = $("comfy-chip");
  const text = `${st.url} · ${st.model} · ${st.online ? (lang === "zh-CN" ? "在线" : "online") : (lang === "zh-CN" ? "离线" : "offline")}`;
  if (line) line.textContent = (st.online ? "[OK] " : "[--] ") + text;
  if (chip) {
    chip.textContent = st.online ? (lang === "zh-CN" ? "ComfyUI 在线" : "ComfyUI online") : (lang === "zh-CN" ? "ComfyUI 离线" : "ComfyUI offline");
    chip.style.color = st.online ? "var(--ok)" : "var(--text-tertiary)";
  }
  return st.online;
}
async function openComfyPanel() {
  openModal("modal-comfy");
  // the AI-drawing dialog mirrors the settings pane's launch path (single
  // source of truth stays in #comfy-launch-path; typing in the dialog syncs back)
  const im = $("imggen-launch-path"), sm = $("comfy-launch-path");
  if (im && sm) im.value = sm.value;
  if (!invoke) {
    $("comfy-status-line").textContent = t("demo_mode");
    return;
  }
  try {
    const st = await invoke("comfy_status");
    $("comfy-url").value = st.url;
    $("comfy-model").value = st.model;
    if ($("comfy-launch-path")) $("comfy-launch-path").value = st.launch_path || "";
    renderComfyStatus(st);
  } catch (e) {
    $("comfy-status-line").textContent = String(e);
  }
}

let currentTpl = "cover";

/* ------------------------------------------------------------- bind UI */
function bindUI() {
  /* editor + preview */
  $("editor").addEventListener("input", () => {
    dirty = true;
    $("stat-saved").textContent = t("not_saved");
    scheduleConvert();
    window.Mozai && Mozai.typing();
  });
  $("editor").addEventListener("paste", async (e) => {
    const items = (e.clipboardData && e.clipboardData.items) || [];
    for (const it of items) {
      if (it.type && it.type.startsWith("image/")) {
        e.preventDefault();
        if (!invoke) { toast(t("demo_mode"), "err"); return; }
        const blob = it.getAsFile();
        if (!blob) return;
        const b64 = await blobToBase64(blob);
        const ext = (it.type.split("/")[1] || "png").replace("jpeg", "jpg");
        try {
          const path = await invoke("import_image_bytes", { filename: `paste-${Date.now()}.${ext}`, base64Data: b64 });
          insertAtCursor(`\n![图片](${path})\n`);
          toast(t("img_imported", 1), "ok");
        } catch (err) {
          toast(String(err), "err");
        }
        return;
      }
    }
  });
  $("theme-select").addEventListener("change", (e) => {
    currentTheme = e.target.value;
    localStorage.setItem("wxwright-theme", currentTheme);
    convertNow();
  });
  $("btn-copy").addEventListener("click", doCopy);
  $("btn-export").addEventListener("click", doExport);
  $("btn-validate").addEventListener("click", doValidate);
  $("rule-chip").addEventListener("click", () => {
    const panel = $("violations-panel");
    panel.hidden = !panel.hidden;
    if (!panel.hidden) renderViolationsPanel((lastResult && lastResult.violations) || []);
  });

  /* device frame */
  document.querySelectorAll(".seg-btn[data-device]").forEach((btn) => {
    btn.addEventListener("click", () => {
      localStorage.setItem("wxwright-device", btn.dataset.device);
      applyDevice();
    });
  });
  $("btn-device-dark").addEventListener("click", toggleDark);
  window.addEventListener("resize", fitDevice);

  /* library */
  $("btn-library").addEventListener("click", () => $("sidebar").classList.toggle("collapsed"));
  $("btn-new-article").addEventListener("click", newArticle);
  $("btn-import").addEventListener("click", importMdFiles);
  $("btn-save").addEventListener("click", () => persistCurrent(false));
  $("library-search").addEventListener("input", (e) => {
    libraryFilter = e.target.value.trim();
    refreshLibrary();
  });
  window.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      persistCurrent(false);
    }
    if (e.key === "Escape") {
      // topmost visible modal closes on Escape; the prompt modal goes through
      // prompt-cancel so a pending uiPrompt promise resolves cleanly
      const promptOv = $("modal-prompt");
      if (promptOv && !promptOv.hidden) {
        if (document.activeElement !== $("prompt-input")) $("prompt-cancel").click();
        return;
      }
      const open = [...document.querySelectorAll(".modal-overlay:not([hidden])")];
      if (open.length) {
        open[open.length - 1].hidden = true;
        e.preventDefault();
      }
    }
  });

  /* AI drawer */
  $("btn-ai").addEventListener("click", () => {
    const d = $("ai-drawer");
    d.hidden = !d.hidden;
    if (!d.hidden) $("ai-input").focus();
  });
  $("btn-ai-close").addEventListener("click", () => { $("ai-drawer").hidden = true; });
  $("btn-ai-clear").addEventListener("click", () => {
    aiStopped = true; // aborts a running demo stream
    aiHistory = [];
    aiBusy = false;
    $("ai-messages").innerHTML = "";
    const sendBtn = $("btn-ai-send");
    sendBtn.innerHTML = '<svg class="icon"><use href="#i-send"/></svg>';
    sendBtn.classList.remove("stopping");
  });
  $("btn-ai-send").addEventListener("click", () => {
    if (aiBusy) {
      aiStopped = true;
      invoke("ai_stop").catch(() => {});
      return;
    }
    const v = $("ai-input").value;
    $("ai-input").value = "";
    autoGrow($("ai-input"));
    aiSend(v);
  });
  $("ai-input").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      const v = $("ai-input").value;
      $("ai-input").value = "";
      autoGrow($("ai-input"));
      aiSend(v);
    }
  });
  $("ai-input").addEventListener("input", (e) => autoGrow(e.target));
  $("btn-ai-attach").addEventListener("click", addAttachment);
  // quick prompts: scoped to the drawer only (theme presets reuse .chip)
  document.querySelectorAll("#ai-drawer .ai-quick .chip[data-prompt]").forEach((chip) => {
    chip.addEventListener("click", () => {
      const kind = chip.dataset.prompt;
      if (kind === "poster") {
        $("poster-desc").value = buildQuickPrompt("poster").split("\n")[0];
        openModal("modal-poster");
        updatePosterPreview();
        return;
      }
      aiSend(buildQuickPrompt(kind));
    });
  });
  const jump = document.createElement("button");
  jump.id = "ai-jump";
  jump.className = "chip ai-jump";
  jump.hidden = true;
  jump.textContent = lang === "zh-CN" ? "回到底部" : "Jump to latest";
  jump.addEventListener("click", () => { msgPinned = true; msgScrollBottom(true); updateJumpChip(); });
  $("ai-messages").insertAdjacentElement("afterend", jump);
  $("ai-messages").addEventListener("scroll", () => {
    msgPinned = msgScrollPinned();
    updateJumpChip();
  });
  $("ai-model-select").addEventListener("click", (e) => {
    e.stopPropagation();
    const menu = $("ai-model-menu");
    const btn = $("ai-model-select");
    if (menu.hidden) {
      renderModelMenu();
      menu.hidden = false;
      $("ai-model-select").classList.add("open");
      // anchor the menu to the trigger button: above when there is room,
      // below otherwise; horizontally aligned to the button, clamped onscreen
      const br = btn.getBoundingClientRect();
      menu.style.visibility = "hidden";
      menu.style.display = "block";
      const mw = menu.offsetWidth || 280;
      const mh = menu.offsetHeight || 260;
      menu.style.display = "";
      menu.style.visibility = "";
      let left = Math.min(Math.max(8, br.left), window.innerWidth - mw - 8);
      let top;
      const spaceAbove = br.top - 8;
      if (spaceAbove >= mh) top = br.top - mh - 6;
      else top = Math.min(br.bottom + 6, window.innerHeight - mh - 8);
      menu.style.left = `${Math.round(left)}px`;
      menu.style.top = `${Math.round(top)}px`;
    } else {
      menu.hidden = true;
      $("ai-model-select").classList.remove("open");
    }
  });
  document.addEventListener("click", (e) => {
    const menu = $("ai-model-menu");
    if (!menu.hidden && !e.target.closest("#ai-model-menu") && !e.target.closest("#ai-model-select")) {
      menu.hidden = true;
      $("ai-model-select").classList.remove("open");
    }
  });

  /* settings modal */
  $("btn-settings").addEventListener("click", async () => {
    openModal("modal-settings");
    renderProviderList();
    refreshWxStatus();
    if (invoke) {
      try {
        const st = await invoke("comfy_status");
        $("comfy-url").value = st.url;
        $("comfy-model").value = st.model;
        renderComfyStatus(st);
      } catch (e) {}
    }
  });
  // settings left-nav pane switching
  document.querySelectorAll("#settings-nav .settings-nav-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      document.querySelectorAll("#settings-nav .settings-nav-btn").forEach((b) =>
        b.classList.toggle("active", b === btn)
      );
      document.querySelectorAll("#modal-settings .settings-pane").forEach((p) =>
        p.classList.toggle("active", p.id === btn.dataset.pane)
      );
    });
  });
  // wechat mp binding: status refresh + bind/unbind
  async function refreshWxStatus() {
    const chip = $("wx-status");
    if (!chip) return;
    if (!invoke) {
      chip.textContent = lang === "zh-CN" ? "浏览器演示：未绑定" : "browser demo: not bound";
      return;
    }
    try {
      const st = await invoke("wx_bind_status");
      chip.textContent = st.bound
        ? lang === "zh-CN" ? `已绑定 · ${st.appid}` : `bound · ${st.appid}`
        : lang === "zh-CN" ? "未绑定" : "not bound";
      chip.style.color = st.bound ? "var(--ok)" : "var(--text-tertiary)";
      $("wx-appid").value = "";
      $("wx-secret").value = "";
    } catch (e) {
      chip.textContent = t("wx_status_fail");
    }
  }
  $("wx-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    if (!invoke) return toast(t("wx_demo_only"), "err");
    const appid = $("wx-appid").value.trim();
    const secret = $("wx-secret").value.trim();
    try {
      const st = await invoke("wx_bind", { appid, secret });
      toast(t("wx_bind_ok"), "ok");
      $("wx-secret").value = "";
      chipSync(st);
    } catch (err) {
      toast(String(err), "err");
    }
    function chipSync(st) {
      const chip = $("wx-status");
      chip.textContent = lang === "zh-CN" ? `已绑定 · ${st.appid}` : `bound · ${st.appid}`;
      chip.style.color = "var(--ok)";
    }
  });
  $("wx-unbind").addEventListener("click", async () => {
    if (!invoke) return;
    try {
      await invoke("wx_unbind");
      toast(t("wx_unbind_ok"), "ok");
      const chip = $("wx-status");
      chip.textContent = lang === "zh-CN" ? "未绑定" : "not bound";
      chip.style.color = "var(--text-tertiary)";
    } catch (err) {
      toast(String(err), "err");
    }
  });
  $("provider-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    await saveProviderFromForm();
    toast(t("provider_saved"), "ok");
    refreshModelSelect();
  });
  // vendor preset dropdown: one click fills name/base_url/model/brand
  const vendorSel = $("pf-vendor");
  for (const [key, v] of Object.entries(VENDOR_PRESETS)) {
    const opt = document.createElement("option");
    opt.value = key;
    opt.textContent = `${v.name} · ${v.base}`;
    vendorSel.appendChild(opt);
  }
  vendorSel.addEventListener("change", () => {
    const v = VENDOR_PRESETS[vendorSel.value];
    if (!v) return;
    $("pf-id").value = "";
    $("pf-name").value = v.name;
    $("pf-base").value = v.base;
    $("pf-model").value = v.model;
    $("pf-brand").value = v.brand;
    $("pf-key").value = "";
    $("pf-logo").value = "";
    renderProviderList();
  });
  $("pf-logo-pick").addEventListener("click", () => {
    const inpEl = document.createElement("input");
    inpEl.type = "file";
    inpEl.accept = "image/png,image/jpeg,image/webp,image/svg+xml";
    inpEl.onchange = () => {
      const f = inpEl.files && inpEl.files[0];
      if (!f) return;
      const r = new FileReader();
      r.onload = () => { $("pf-logo").value = String(r.result); };
      r.readAsDataURL(f);
    };
    inpEl.click();
  });
  $("pf-logo-clear").addEventListener("click", () => { $("pf-logo").value = ""; });
  $("pf-cancel").addEventListener("click", () => {
    ["pf-id", "pf-name", "pf-model", "pf-base", "pf-key", "pf-logo"].forEach((id) => ($(id).value = ""));
  });
  // Fixed: only save when the form is complete; otherwise test the active provider.
  $("pf-test").addEventListener("click", async () => {
    try {
      const name = $("pf-name").value.trim();
      const base = $("pf-base").value.trim();
      const model = $("pf-model").value.trim();
      let id;
      if (name && base && model) {
        const st = await saveProviderFromForm();
        id = st.active;
      } else {
        const st = await invoke("ai_settings");
        if (!st.active) throw new Error(lang === "zh-CN" ? "表单未填写，且没有已启用的 Provider" : "Form is empty and no active provider");
        id = st.active;
      }
      $("pf-test").disabled = true;
      const reply = await invoke("ai_test", { id });
      toast(t("connected") + reply, "ok");
    } catch (e) {
      toast(String(e), "err");
    } finally {
      $("pf-test").disabled = false;
    }
  });

  /* ComfyUI: config / launch / image source */
  const comfySave = async () => {
    try {
      const st = await invoke("comfy_save_config", {
        url: $("comfy-url").value,
        model: $("comfy-model").value,
        launchPath: $("comfy-launch-path").value,
      });
      renderComfyStatus(st);
      toast(lang === "zh-CN" ? "ComfyUI 配置已保存" : "ComfyUI config saved", "ok");
    } catch (e) {
      toast(String(e), "err");
    }
  };
  $("comfy-save").addEventListener("click", comfySave);
  const comfyStart = async () => {
    const btns = [$("comfy-start")];
    btns.forEach((b) => b && (b.disabled = true));
    if (invoke) {
      try {
        await invoke("comfy_save_config", {
          url: $("comfy-url").value,
          model: $("comfy-model").value,
          launchPath: $("comfy-launch-path").value,
        });
      } catch (e) {}
    }
    if (!invoke) { toast(t("demo_mode"), "err"); btns.forEach((b) => b && (b.disabled = false)); return; }
    $("comfy-progress").textContent = lang === "zh-CN" ? "正在启动 ComfyUI..." : "Starting ComfyUI...";
    try {
      const st = await invoke("comfy_launch", { launchPath: $("comfy-launch-path").value });
      renderComfyStatus(st);
      toast(lang === "zh-CN" ? "ComfyUI 已在线" : "ComfyUI online", "ok");
    } catch (e) {
      toast(String(e), "err");
    } finally {
      btns.forEach((b) => b && (b.disabled = false));
      $("comfy-progress").textContent = "";
    }
  };
  const sBtn = document.getElementById("comfy-start");
  if (sBtn) sBtn.addEventListener("click", comfyStart);
  const sBtn2 = document.getElementById("imggen-comfy-start");
  if (sBtn2) sBtn2.addEventListener("click", comfyStart);
  // dialog launch-path mirrors back into the settings pane input
  const imlp = $("imggen-launch-path"), smlp = $("comfy-launch-path");
  if (imlp && smlp) imlp.addEventListener("input", () => { smlp.value = imlp.value; });

  /* agent modal */
  $("btn-agent").addEventListener("click", () => { openModal("modal-agent"); loadAgentCard(); });
  $("btn-copy-card").addEventListener("click", async () => {
    if (invoke) {
      await invoke("copy_agent_card");
    } else {
      const pre = $("agent-card-pre");
      if (pre.dataset.loaded !== "1") await loadAgentCard();
      await copyPlain(pre.textContent);
    }
    toast(t("copied_text"), "ok");
  });
  $("btn-toggle-card").addEventListener("click", async () => {
    const pre = $("agent-card-pre");
    await loadAgentCard();
    pre.hidden = !pre.hidden;
    $("btn-toggle-card").textContent = pre.hidden ? t("preview_card") : t("hide_card");
  });
  document.querySelectorAll(".mcp-btn").forEach((btn) => {
    btn.addEventListener("click", async () => {
      if (!invoke) { toast(t("demo_mode"), "err"); return; }
      try {
        const o = await invoke("mcp_install", { target: btn.dataset.target });
        const r = $("mcp-result");
        r.hidden = false;
        r.textContent = `${o.config_path}  (key: ${o.key}${o.backup_created ? ", .bak" : ""})`;
        toast(t("mcp_installed", o.config_path), "ok");
      } catch (e) {
        toast(String(e), "err");
      }
    });
  });
  document.querySelectorAll(".cli-row").forEach((row) => {
    row.addEventListener("click", async () => {
      await copyPlain(row.dataset.cmd);
      toast(t("copied_text"), "ok");
    });
  });

  /* theme AI modal */
  $("btn-theme-ai").addEventListener("click", () => openModal("modal-theme"));
  document.querySelectorAll(".theme-preset").forEach((chip) => {
    chip.addEventListener("click", () => { $("theme-desc").value = chip.dataset.desc; });
  });
  $("btn-theme-generate").addEventListener("click", generateTheme);

  /* poster modal */
  $("btn-poster").addEventListener("click", () => {
    if (!$("poster-html").value.trim()) {
      const { w, h } = posterSize();
      $("poster-html").value = posterTemplate(w, h, "cover");
      posterTemplateDirty = false;
    }
    openModal("modal-poster");
    updatePosterPreview();
  });
  $("poster-html").addEventListener("input", () => { posterTemplateDirty = true; });
  $("poster-preset").addEventListener("change", () => {
    $("poster-custom").hidden = $("poster-preset").value !== "custom";
    updatePosterPreview();
  });
  $("btn-poster-ai").addEventListener("click", posterAiGenerate);
  $("btn-poster-reset").addEventListener("click", () => {
    const { w, h } = posterSize();
    $("poster-html").value = posterTemplate(w, h, currentTpl);
    posterTemplateDirty = false;
    updatePosterPreview();
  });
  $("btn-poster-insert").addEventListener("click", () => posterExport(true));
  $("btn-poster-save").addEventListener("click", () => posterExport(false));
  $("btn-poster-imgset").addEventListener("click", exportImageSet);
  $("poster-html").addEventListener("input", () => {
    clearTimeout($("poster-html")._t);
    $("poster-html")._t = setTimeout(updatePosterPreview, 400);
  });
  window.addEventListener("resize", () => {
    if (!$("modal-poster").hidden) updatePosterPreview();
  });
  if (window.ResizeObserver) {
    new ResizeObserver(() => {
      if (!$("modal-poster").hidden) updatePosterPreview();
    }).observe($("poster-preview-box"));
  }
  document.querySelectorAll(".tpl-chip").forEach((chip) => {
    chip.addEventListener("click", () => {
      currentTpl = chip.dataset.tpl;
      document.querySelectorAll(".tpl-chip").forEach((c) => c.classList.toggle("active", c === chip));
      const preset = { cover: "1080x460", quote: "1080x1440", pic: "1080x1440" }[currentTpl] || "1080x460";
      $("poster-preset").value = preset;
      $("poster-custom").hidden = preset !== "custom";
      const { w, h } = posterSize();
      $("poster-html").value = posterTemplate(w, h, currentTpl);
      updatePosterPreview();
    });
  });

  /* fit studio */
  $("fit-preset").addEventListener("change", fitDraw);
  $("fit-mode").addEventListener("change", () => {
    $("fit-bg-row").hidden = $("fit-mode").value !== "contain";
    fitDraw();
  });
  $("fit-scale").addEventListener("change", fitDraw);
  $("fit-bg").addEventListener("input", fitDraw);
  document.querySelectorAll("#fit-bg-row .chip[data-bg]").forEach((chip) => {
    chip.addEventListener("click", () => { $("fit-bg").value = chip.dataset.bg; fitDraw(); });
  });
  $("fit-export").addEventListener("click", fitExport);

  /* assets panel */
  $("btn-assets").addEventListener("click", () => openAssetsPanel());
  $("assets-open-folder").addEventListener("click", () => {
    if (window.__TAURI__ && window.__TAURI__.opener) {
      invoke("library_dir").then((d) => {
        const dir = d.replace(/articles$/, "assets");
        window.__TAURI__.opener.openPath(dir).catch(() => {
          window.__TAURI__.opener.revealItemInDir(dir).catch(() => {});
        });
      });
    } else {
      toast(t("demo_mode"), "err");
    }
  });

  /* comfy dialog */
  $("btn-comfy").addEventListener("click", () => { openComfyPanel(); });
  document.querySelectorAll("#modal-comfy .chip[data-mode]").forEach((chip) => {
    chip.addEventListener("click", () => setComfyMode(chip.dataset.mode));
  });
  document.querySelectorAll("#modal-comfy .seg-btn[data-imgsrc]").forEach((btn) => {
    btn.addEventListener("click", () => setComfyImgSrc(btn.dataset.imgsrc));
  });
  $("comfy-cloud-save").addEventListener("click", async () => {
    if (!invoke) { toast(t("demo_mode"), "err"); return; }
    try {
      await invoke("ai_save_image_model", { model: $("comfy-cloud-model").value });
      toast(lang === "zh-CN" ? "图像模型已保存" : "Image model saved", "ok");
    } catch (e) {
      toast(String(e), "err");
    }
  });
  $("comfy-pick").addEventListener("click", () => openAssetsPanel({ selectMode: true }));
  $("comfy-generate").addEventListener("click", comfyGenerate);
  const mStart = document.getElementById("comfy-start-modal");
  if (mStart) mStart.addEventListener("click", comfyStart);

  /* svg kit modal */
  $("btn-svgkit").addEventListener("click", () => { openModal("modal-svgkit"); renderSvgKit(); });
  $("svgkit-insert").addEventListener("click", () => {
    const svg = currentSvgMarkup();
    if (!svg) { toast(lang === "zh-CN" ? "当前组件为空" : "Nothing to insert", "err"); return; }
    // Block-level embed: append as a standalone block at the end (cursor
    // insertion would nest it inside lists/code and break the classification)
    const ed = $("editor");
    ed.value = ed.value.replace(/\s*$/, "") + "\n\n" + svg + "\n";
    ed.selectionStart = ed.selectionEnd = ed.value.length;
    dirty = true;
    $("stat-saved").textContent = t("not_saved");
    if (window.Mozai) Mozai.typing();
    scheduleConvert();
    closeModal("modal-svgkit");
    toast(lang === "zh-CN" ? "组件已插入（引擎已复核合规）" : "Inserted (re-validated by the engine)", "ok");
    if (window.Mozai) Mozai.celebrate();
  });
  $("svgkit-ai-generate").addEventListener("click", svgKitAiGenerate);

  /* modal close */
  document.querySelectorAll(".modal-close").forEach((btn) => {
    btn.addEventListener("click", () => closeModal(btn.dataset.close));
  });
  document.querySelectorAll(".modal-overlay").forEach((ov) => {
    ov.addEventListener("mousedown", (e) => { if (e.target === ov) ov.hidden = true; });
  });

  /* drag & drop */
  if (listen) {
    listen("dropped-files", async (ev) => {
      const paths = ev.payload || [];
      const handled = await importDroppedImages(paths);
      if (handled) return;
      const DOCS = ["txt", "pdf", "docx", "html", "htm", "csv", "json", "xml"];
      const docFiles = paths.filter((p) => {
        const ext = p.toLowerCase().includes(".") ? p.toLowerCase().split(".").pop() : "";
        return DOCS.includes(ext);
      });
      for (const df of docFiles) {
        try {
          if (pendingAttachments.length >= 4) { toast(lang === "zh-CN" ? "最多 4 个附件" : "Max 4 attachments", "err"); break; }
          const doc = await invoke("extract_document_text", { path: df });
          pendingAttachments.push({ kind: "text", name: df.split(/[\/]/).pop(), content: doc.text, truncated: doc.truncated });
          renderAttachRow();
          toast(lang === "zh-CN" ? "已加入 AI 附件" : "Added as AI attachment", "ok");
        } catch (e) { toast(String(e), "err"); }
      }
      if (docFiles.length) return;
      const mdFile = paths.find((p) => p.toLowerCase().endsWith(".md") || p.toLowerCase().endsWith(".markdown"));
      if (mdFile) {
        try {
          if (dirty) await persistCurrent(true);
          $("editor").value = await invoke("read_text_file", { path: mdFile });
          currentArticleId = null;
          dirty = false;
          $("stat-saved").textContent = t("not_saved");
          scheduleConvert();
        } catch (e) {
          toast(String(e), "err");
        }
      }
    });
  }

  /* splitter */
  const splitter = $("splitter");
  let dragging = false;
  splitter.addEventListener("mousedown", () => { dragging = true; splitter.classList.add("dragging"); });
  window.addEventListener("mouseup", () => { dragging = false; splitter.classList.remove("dragging"); });
  window.addEventListener("mousemove", (e) => {
    if (!dragging) return;
    const total = $("workspace").getBoundingClientRect().width;
    const left = Math.min(Math.max(e.clientX / total, 0.2), 0.78);
    $("workspace").querySelector(".pane-editor").style.flex = `0 0 ${left * 100}%`;
  });

  /* language toggle: swap locale, then re-render every dynamic surface */
  $("btn-lang").addEventListener("click", () => {
    lang = lang === "zh-CN" ? "en" : "zh-CN";
    localStorage.setItem("wxwright-lang", lang);
    applyI18n();
    refreshLibrary();
    refreshModelSelect();
    renderPlatformOptions();
    renderPlatformPresets();
    refreshCopyButton();
    convertNow();
  });

  /* QR code modal */
  $("malong-btn").addEventListener("click", () => openModal("modal-qr"));

  /* easter egg: click the wxwright logo three times quickly -> Mozai party */
  let logoClicks = 0, logoTimer = null;
  $("logo-btn").addEventListener("click", () => {
    logoClicks++;
    clearTimeout(logoTimer);
    logoTimer = setTimeout(() => { logoClicks = 0; }, 1500);
    if (logoClicks >= 3) {
      logoClicks = 0;
      window.Mozai && Mozai.celebrate();
      toast(lang === "zh-CN" ? "彩蛋：墨仔来派对啦！公众号「码聋」见。" : "Easter egg: Mozai party! See you at MP 码聋.", "ok");
    }
  });

  /* github link */
  $("link-github").addEventListener("click", (e) => {
    e.preventDefault();
    if (window.__TAURI__ && window.__TAURI__.opener) {
      window.__TAURI__.opener.openUrl("https://github.com/YaoIsAI");
    } else {
      window.open("https://github.com/YaoIsAI", "_blank");
    }
  });

}

/* ------------------------------------------------------ comfy generation */
async function comfyGenerate() {
  if (jobBusy.comfy) return;
  if (!invoke) { toast(t("demo_mode"), "err"); return; }
  const prompt = $("comfy-prompt").value.trim();
  if (!prompt) return;
  const [w, h] = $("comfy-size").value.split("x").map((x) => parseInt(x));
  const steps = Math.max(4, parseInt($("comfy-steps").value) || 20);
  $("comfy-progress").textContent =
    comfyImgSrc === "cloud"
      ? lang === "zh-CN" ? "云端生成中..." : "Generating in the cloud..."
      : lang === "zh-CN" ? "已提交 ComfyUI 队列，生成中（最多等待 10 分钟）..." : "Queued in ComfyUI (up to 10 min)...";
  $("comfy-results").innerHTML = "";
  jobBusy.comfy = true;
  try {
    let params;
    if (comfyImgSrc === "cloud") {
      params = { mode: "cloud", prompt, width: w, height: h };
    } else if (comfyMode === "i2i") {
      const src = $("comfy-source").value.trim();
      if (!src) throw new Error(lang === "zh-CN" ? "请先从素材库选择源图" : "Pick a source image first");
      params = { mode: "i2i", sourcePath: src, prompt, negative: $("comfy-negative").value, denoise: parseFloat($("comfy-denoise").value) || 0.55, steps };
    } else {
      params = { mode: "t2i", prompt, negative: $("comfy-negative").value, width: w, height: h, steps };
    }
    const job = await AIJobs.run("comfy", params, { button: $("comfy-generate") });
    if (job.stopped) { $("comfy-progress").textContent = t("job_stopped"); return; }
    const paths = job.result.paths;
    renderComfyResults(paths);
    $("comfy-progress").textContent = "";
    toast(paths.length === 1
      ? (lang === "zh-CN" ? "生成完成，点击图片插入文章" : "Done, click the image to insert it")
      : (lang === "zh-CN" ? `生成完成（${paths.length} 张）` : `Done (${paths.length})`), "ok");
    window.Mozai && Mozai.celebrate();
  } catch (e) {
    $("comfy-progress").textContent = "";
    toast(String(e), "err");
  } finally {
    delete jobBusy.comfy;
  }
}

function renderComfyResults(paths) {
  for (const p of paths) {
    const item = document.createElement("div");
    item.className = "asset-item";
    item.innerHTML = `<img alt="result" /><div class="a-meta">${escapeHtml(p.split(/[\\/]/).pop())}</div>
      <div class="a-tools"><button type="button" class="ins" title="${t("insert_article")}"><svg class="icon icon-sm"><use href="#i-plus"/></svg></button></div>`;
    invoke("asset_thumb", { path: p, size: 320 }).then((uri) => { item.querySelector("img").src = uri; }).catch(() => {});
    item.querySelector(".ins").addEventListener("click", () => {
      insertAtCursor(`\n![AI 绘图](${p})\n`);
      toast(t("poster_inserted"), "ok");
    });
    item.addEventListener("click", () => {
      insertAtCursor(`\n![AI 绘图](${p})\n`);
      toast(t("poster_inserted"), "ok");
    });
    $("comfy-results").appendChild(item);
  }
}

/* ------------------------------------------------------ init (crash-proof) */
async function init() {
  lang = localStorage.getItem("wxwright-lang") || "zh-CN";
  applyI18n();
  currentTheme = localStorage.getItem("wxwright-theme") || "minimal";

  // 1. bind every listener first: a failing async load can no longer leave
  //    the UI half-wired (the reported blank-open + dead quick-chips bug).
  try {
    bindUI();
  } catch (e) {
    console.error("bindUI failed:", e);
    toast(`${lang === "zh-CN" ? "界面初始化失败" : "UI init failed"}: ${e.message}`, "err");
  }

  if (!invoke) {
    // browser demo mode: mirror the three core built-in themes
    $("theme-select").innerHTML = [
      ["minimal", lang === "zh-CN" ? "素黑" : "Minimal"],
      ["techblue", lang === "zh-CN" ? "科技蓝" : "Tech Blue"],
      ["magazine", lang === "zh-CN" ? "杂志" : "Magazine"],
    ].map(([id, label]) => `<option value="${id}">${label}</option>`).join("");
    if (![...$("theme-select").options].some((o) => o.value === currentTheme)) currentTheme = "minimal";
    $("theme-select").value = currentTheme;
    try {
      const [demo, demoMd] = await Promise.all([
        fetch("demo-preview.html").then((r) => r.text()),
        fetch("demo-article.md").then((r) => r.text()),
      ]);
      $("preview").srcdoc = demo;
      $("editor").value = demoMd;
    } catch (e) {}
    await initPlatformSwitcher();
    refreshLibrary();
    refreshModelSelect();
    applyDevice();
    window.Mozai && Mozai.setState("idle");
    await convertNow();
    return;
  }

  // 2. async loads, each isolated: one failure never kills the rest
  const steps = [
    async () => {
      $("editor").value = await invoke("load_sample");
      currentArticleId = null;
      loadedTitle = null;
    },
    async () => {
      const themes = await invoke("list_themes");
      $("theme-select").innerHTML = themes
        .map((x) => `<option value="${x.id}">${escapeHtml(lang === "zh-CN" ? x.name_zh || x.name : x.name)}</option>`)
        .join("");
      $("theme-select").value = currentTheme;
    },
    () => initPlatformSwitcher(),
    () => refreshLibrary(),
    () => refreshModelSelect(),
    async () => {
      const st = await invoke("comfy_status");
      $("comfy-url").value = st.url;
      $("comfy-model").value = st.model;
      $("comfy-launch-path").value = st.launch_path || "";
      renderComfyStatus(st);
    },
    async () => {
      const im = await invoke("ai_settings").then((x) => x.image_model).catch(() => "");
      if (im) $("comfy-cloud-model").value = im;
    },
    async () => {
      const p = await invoke("library_dir");
      $("library-path").textContent = p;
      $("library-path").title = p;
    },
  ];
  for (const step of steps) {
    try {
      await step();
    } catch (e) {
      console.error("init step failed:", e);
      toast(`${lang === "zh-CN" ? "初始化部分失败" : "Init step failed"}: ${e}`, "err");
    }
  }

  if (!localStorage.getItem("wxwright-hint")) {
    setTimeout(() => toast(t("first_run_hint")), 1200);
    localStorage.setItem("wxwright-hint", "1");
  }

  // 3. first render + one retry against startup races (blank-open fix)
  await convertNow();
  if (!lastResult) {
    setTimeout(async () => {
      try { await convertNow(); } catch (e) {}
    }, 900);
  }
  applyDevice();
  window.Mozai && Mozai.setState("idle");
}

init();
