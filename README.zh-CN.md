# wxwright

让任何 AI Agent（或人类）用一条命令、一次 MCP 调用、一次粘贴，把 Markdown 变成零样式失真的微信公众号文章。

Rust · CLI · MCP · 桌面客户端 · 单二进制

> English: see [README.md](README.md)

## 30 秒上手

```bash
# 1. 转换（无需任何凭据）
wxwright convert article.md --theme minimal --out article.html

# 2. 剪贴板全链路：转换 -> 图片 -> 规范化 -> 校验 -> 写入剪贴板
wxwright copy article.md
# 然后打开公众号编辑器，Ctrl+V 粘贴
```

## 面向 AI Agent

把 `wxwright agent-card --md` 输出的整段卡片贴进任意 Agent 的系统提示，Agent 即刻接手。机读合同：`wxwright agent-card --json`。

MCP 一行接入：

```bash
wxwright mcp install --target claude   # 或 cursor | vscode | opencode
wxwright mcp serve                     # stdio MCP server
```

工具：`wxwright_convert`、`wxwright_validate`、`wxwright_copy`、`wxwright_themes_list`、`wxwright_draft_create`、`wxwright_draft_list`。资源：`wxwright://themes`、`wxwright://spec/rules`（完整规则表，中英双语）。提示：`wxwright-publish-guide`。

退出码：`0` 通过 · `1` 违规 · `2` 运行异常。所有命令支持 `--json`（稳定 schema，非 TTY 默认 JSON）与 stdin（`-`）。

## 为什么不同

公众号编辑器是加固版 ProseMirror：外部样式表被丢弃、class/id 失效、大量"魔法样式"（opacity 藏图、固定像素宽、height:0 折叠）在编辑器里正常、发布后或移动端直接坏掉。多数转换器只对齐编辑器视图；wxwright 对齐全部四视图：编辑器、PC 已发布、移动端、Dark Mode。

1. **引擎而非脚本** —— 一个纯 Rust core（`wxwright-core`），CLI / MCP / GUI 共享；未来移动端 FFI 复用同一颗大脑。
2. **先生成正确的，再校验** —— 主题直接渲染成官方方言（`section` 块级 + `span[leaf]` 行内、全内联样式）；normalizer 与 validator 是第二道防线。
3. **全量规则引擎** —— 官方规范每条规则（R-1.1 … R-4.4）都有生成策略 + 自动修复 + 检测，每条至少一个违规样例测试；内置校验器与官方 `verify-article-structure-spec`（puppeteer CLI）对齐，后者作为 CI 真值门禁。
4. **本地极致性能** —— 单静态二进制、零守护、零 IPC。Release SLO：万字全链路 p50 ≤ 60ms、CLI 单平台 ≤ 8MB（见 `docs/performance.md`）。

## 零失真的四个条件

`wxwright copy` 只有在四条同时满足时才会成功（PRD §3.5）：

1. 载荷正确：自包含 `text/html`，全内联样式，无 `<style>`/`class`/`script`/外部字体；
2. 结构正确：HTML 本身就长在官方白名单内；
3. 图片正确：mmbiz 域（或明确警告的 https 直链）；本地/base64 图片会被**阻断并列出清单**——粘贴必挂；
4. 真值验证：内置校验器 + CI 中的官方 CLI 桥。

配置凭据（`wxwright login`）后，本地图片自动上传永久素材并改写为 mmbiz；`wxwright draft create` 直接写入草稿箱 (Drafts)。

## 主题

内置三套：`minimal` 素黑 / `techblue` 科技蓝 / `magazine` 杂志。主题即数据：TOML 色板 + 可选的角色样式覆盖，渲染期展开为内联样式。`wxwright theme new my-theme` 生成脚手架；`wxwright theme validate` 保证其产出合规后才可进仓库。全引擎禁用 `font-family`（官方规则 R-3.1）——主题无法夹带。

组件（Markdown 原生触发）：GitHub alert 语法 `> [!NOTE]` / `[!TIP]` / `[!IMPORTANT]` / `[!WARNING]` / `[!CAUTION]` 渲染为提示卡；`> [!COMMENT]` 留言卡；`> [!KEYPOINT] 文字` 划重点卡；`[TOC]` 目录卡；图片下一行斜体自动成图注；标题支持 `{.center}` 居中。

## CLI 参考

```
wxwright convert  <input.md|->   --theme <名称|路径>  --out <file.html|->  [--json]
wxwright validate <input.md|html> [--json] [--strict] [--official-check]
wxwright fix      <input.html>   --out <fixed.html>
wxwright copy     <input.md>     --theme <名称> [--dry-run]
wxwright image    upload <paths...>
wxwright draft    create|update|list            （需凭据）
wxwright publish  <draft_id> --yes              （群发；显式确认）
wxwright theme    list|new|validate
wxwright doctor
wxwright mcp      serve | install --target <claude|cursor|vscode|opencode>
wxwright agent-card [--md|--json]
wxwright login --appid <id> --secret <key>     # 存入系统钥匙串
wxwright logout
wxwright bench
```

## 桌面客户端

`wxwright-gui`（Tauri 2）：

- **三栏布局**：文章库（左）/ 编辑器（中）/ 手机框预览（右），Dark Mode 模拟、一键复制富文本、导出 HTML、拖拽载入 .md。
- **文章库**：本地 Markdown 存储（`文档/wxwright/articles`，frontmatter 元数据）；切换自动保存、点击重新打开、删除、新建。
- **AI 助手**：编辑器下方 Codex 式对话抽屉 - 流式输出，支持任意 OpenAI 兼容 Provider（OpenAI / DeepSeek / 通义 / Kimi / 智谱，或本地 Ollama / LM Studio）。API Key 存系统钥匙串。快捷指令：润色 / 续写 / 起标题 / 提纲；每条回复可一键插入 / 替换 / 复制。
- **Agent 接入面板**（机器人图标）：一键写入 Claude Desktop / Cursor / VS Code / OpenCode 的 MCP 配置、一键复制 Agent 卡片、CLI 速查表。同一引擎，同一合规约束。
- **真实手机样机**：iPhone 15 Pro（灵动岛、实体侧键、Home 条）与 Pixel 8（打孔摄像头）按真实硬件绘制，样机下方双按钮切换，深浅色切换在旁；预览保持真实逻辑分辨率并自适应缩放。
- **素材库**：海报导出、AI 绘图、粘贴截图、拖入图片统一进入网格化管理（`文档\wxwright\assets`）：缩略图浏览、一键插入文章、删除。
- **ComfyUI 本地 AI 绘图**：自动检测本机 ComfyUI（默认 `127.0.0.1:8188`，设置中可改）。检测到即可调用本地 Stable Diffusion 文生图 / 图生图（队列 + 轮询 + 取图），产物直接进素材库与文章。零云端、零密钥。
- **规范徽标**常驻字符统计行（彩色阻断/提示计数），明细面板不再遮挡 AI 助手。
- **写作宠物墨仔**：一只真正的坐姿猫（胡须、眨眼、卷尾）。打字时弹跳、45 秒入睡、点击冒爱心、连点 logo 三次触发彩蛋；点击状态栏「码聋」弹出公众号二维码。

GUI 二进制同时响应 `wxwright-gui.exe mcp serve`，单独安装 GUI 也能充当 MCP server。

## 安全

- AppSecret 存系统钥匙串（Windows 凭据管理器 / macOS Keychain / libsecret），配置文件只存引用；`WXWRIGHT_MP_APPID` / `WXWRIGHT_MP_SECRET` 环境变量供 CI 覆盖。
- 日志中 access_token 恒为掩码。
- `publish`（群发）必须显式 `--yes`；默认写路径是草稿箱。
- 零遥测、零云端依赖。

## 已知限制（v1）

- 公式以样式化文本卡片保真（图片模式需要 KaTeX 桥，规划中）。
- `R-2.1` 深层同样式嵌套链：可检测，不自动展开。
- `--official-check` 输出指引与 CI 命令；puppeteer 实际执行在 CI。
- 剪贴板富文本（text/html）在 Windows/macOS/Linux 三端均尝试写入（arboard 3.6），仅在剪贴板无法接受 HTML 时（如无头 CI）降级纯文本。
- clap 内置 help 为英文；报告文案已双语（en / zh-CN）。

## 作者

**AI瑶** - 微信公众号：**码聋** · [github.com/YaoIsAI](https://github.com/YaoIsAI)

GUI 状态栏里住着写作宠物「墨仔」，记得去摸摸它；点击状态栏「码聋」可弹出公众号二维码。

## 自动发布

打 tag（`git tag v1.0.1 && git push origin v1.0.1`）即可由 GitHub Actions 自动构建全平台产物（Windows NSIS 安装包 + 便携 exe、macOS dmg universal + 双架构 CLI、Linux deb + AppImage + musl 静态 CLI + SHA256SUMS）并挂到 Release。详见 `docs/release-automation.md`。

## 许可证

MIT OR Apache-2.0
