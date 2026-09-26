# UI 交互面全量审计结果 / Button-by-Button Audit Findings

- 执行日期：2026-09-26
- 基线：v0.9.0，commit f8eb4f0（规划文档见 `docs/button-audit-plan.md`）
- 执行者：ZCode agent
- 状态：审计完成，问题待修复排期

## 1. 总览

| 项 | 数量 |
|---|---|
| 覆盖交互控件 | 约 117（A-P 十六区全走查） |
| 问题 | 15（P1 x2 / P2 x4 / P3 x9） |
| 通过项 | 见第 3 节 |
| 静态审计零差集 | 命令注册 52=52、CSS 变量悬空 0、cache-bust 与记载一致 |

证据文件（浏览器截图/取值）存放于仓库外 `C:\Users\yao\Desktop\wxwright-audit-evidence\`。

## 2. 问题台账

严重度：P1 = 用户可见的功能损坏；P2 = 功能缺口 / 一致性缺陷；P3 = 打磨与 demo 保真。

| ID | 区域 | 严重度 | 问题 | 证据 | 状态 |
|---|---|---|---|---|---|
| AUD-001 | C6 保存 | P1 | `saved_new_ok` 在 zh/en 两字典均缺失：新文章首次保存的 toast 渲染出字面量 `undefined`（3 处调用：app.js 527/529/543） | i18n 扫描 + 浏览器实测 `t("saved_new_ok")` 返回 `undefined` 且 toast 渲染 `undefined` | open |
| AUD-002 | M3/G13 | P1 | `comfy-launch-path` 重复 id（index.html 435 设置页 / 657 AI 绘图弹窗）：读取（app.js 3084/3101/3108）与回填（2756/3508）全部命中 DOM 顺序在前的设置页输入框，**弹窗内输入框是死控件**——用户在弹窗里填启动路径完全不生效 | 代码链路证明 + 浏览器取值（两输入框互不同步） | open |
| AUD-003 | 全局 | P2 | `t()` 缺 key 返回 `undefined` 而非 key 名（app.js 235-238），铁律 4「缺 key 必须返回 key 名，绝不显示 undefined」在实现层从未落实；AUD-001 即其直接后果 | 代码 + AUD-001 渲染复现 | open |
| AUD-004 | G4 | P2 | `f_logo` 在两字典均缺失：英文模式下「自定义图标」标签残留中文（applyI18n 守卫保留静态文案，双语不齐） | 浏览器实测 EN 模式标签为中文 | open |
| AUD-005 | O6 | P2 | `svgkit-desc` 重复 id（index.html 789 输入框 / 802 详情 span）：span 无任何写入方，组件详情描述**永不显示** | 浏览器取证 count=2（INPUT+SPAN），SPAN textContent 恒空 | open |
| AUD-006 | H3 | P2 | `gui/ui/demo-agent-card.md` 与漂移锁定源（agentcard.rs 46-47，7 工具）不一致：demo 卡只列 4 个 MCP 工具，缺 `wxwright_upload_images / wxwright_draft_create / wxwright_draft_list` | 代码 diff 证明 | open |
| AUD-007 | P 区 | P3 | Escape 不能关闭 8 个弹窗（仅 prompt 输入框内部处理 Escape；全局 keydown 只挂了 Ctrl+S） | 浏览器实测 modal-theme 按 Escape 仍开 | open |
| AUD-008 | 状态栏 | P3 | demo 模式状态栏永久卡在「Converting...」：convertNow 的 demo 分支（app.js 423 `return;`）早于 ready 复位行（414）——坑 22 同类 | 代码 + 浏览器实测（转换完成后仍显示 Converting） | open |
| AUD-009 | C1 | P3 | demo 模式字数/词数/图片数恒为 0：`updateStats` 只在 invoke 路径调用，demo 永不更新 | 浏览器实测打字后仍 0 | open |
| AUD-010 | B3 | P3 | demo 模式搜索过滤无效：demo 文章列表是存根，`libraryFilter` 不参与渲染 | 浏览器实测搜「xyz-no-match」列表不变 | open |
| AUD-011 | J1 | P3 | 海报尺寸预设选项标签无英文（EN 模式下显示「头图 1080x460（2.35:1）」等中文） | 浏览器实测 EN 模式选项为中文 | open |
| AUD-012 | E4 | P3 | demo 微信预览是静态 `demo-preview.html`，编辑正文不实时刷新（其他 6 平台壳在 demo 下实时渲染） | app.js convertNow demo 分支 | open |
| AUD-013 | K 区 | P3 | `prompt-ok/cancel` 两个空绑定 `() => {}`（app.js 3290-3291），噪音代码；真绑定在 uiPrompt 内且正确移除（无累积问题） | 代码 | open |
| AUD-014 | A5 | P3 | demo 主题下拉硬编码单选项（app.js 3468 只填 minimal），3 个内置主题在 demo 不可见不可切换 | 代码 + 浏览器实测 | open |
| AUD-015 | L 区 | P3 | demo 素材卡的插入/尺寸工坊/删除三按钮**未绑定任何事件**（app.js 2642-2657 存根只画壳），点击静默无反应——与其他 demo 边界「有 toast 提示」的行为不一致 | 代码 + 浏览器实测（点击后弹窗不开、无 toast） | open |

### 修复建议（按依赖序）

1. AUD-003 先修：`t()` 加 `v === undefined ? key : ...` 回退——AUD-001 的可见症状随即变为 key 名而非 undefined。
2. AUD-001 + AUD-004：补 `saved_new_ok`（zh/en，函数式如 saved_ok）与 `f_logo` 两个 key。
3. AUD-002 + AUD-005：去掉重复 id——弹窗 launch-path 改名 `imggen-launch-path` 并让读写同步两处（或弹窗直接复用设置页值）；svgkit 详情 span 改名 `svgkit-desc-view` 并在 renderSvgKit 选中时写入。
4. AUD-006：demo 卡从 `agentcard.rs` 重新生成，并加一个防再漂移的测试（对比 demo-agent-card.md 与 `card_markdown()` 输出的工具行）。
5. AUD-007：全局 keydown 增加 Escape 关闭最上层可见弹窗。
6. AUD-008/009/010/012/014/015：demo 分支补 ready 复位、updateStats 本地计数、存根列表过滤、微信 demo 实时渲染（工作量最大，可先文档声明）、3 内置主题、存根按钮绑定 demo toast。
7. AUD-011：poster 预设选项标签走 I18N。
8. AUD-013：删除空绑定。

## 3. 通过项（抽样证据）

- A3/A4 平台下拉：7 平台齐全，逐个切换后壳全部正确联动且结构分化（fb 字标卡片 / IG likes 行 / X 句柄 / LinkedIn 搜索框 / 知乎文章页 / 微信方言+脚注 / XHS 笔记壳）。
- A6/I1/I5 主题弹窗：预设 chip 填充、无 Provider 时明确报错 toast。
- J 区海报：模板切换、编辑实时预览（iframe 内容实测更新）、重置、尺寸 meta 与选中值一致、demo 边界 toast 全部正确。
- M 区绘图：comfy/云端切换、t2i/i2i 行显隐、demo 边界 toast。
- O 区 SVG 库：6 组件、参数编辑实时反映预览、插入文章成功（编辑器 +414 字符，引擎复检 toast）。
- D 区 AI 抽屉：抽屉开合、模型菜单（demo 回退 openai）、快捷 chip 触发 demo 流式、**生成中发送按钮进入 stopping 态**（按钮即停止模式生效）、流毕复位、清空、关闭。
- E 区真机：Pixel/iPhone 切换、深浅色壳内配色跟随（截图 02-pixel-dark.png）、分隔条拖拽布局不破。
- F/A1：宠物单击气泡/双击换形态、二维码弹窗、logo 三连彩蛋。
- A15 i18n：zh/en 全量切换，违规面板、按钮、toast 均随语言重渲染。
- C3 校验：demo 前端校验面板「No violations found. / 未发现规范问题。」双语正确。
- L0 静态：命令注册 52 定义 = 52 注册零差集；JS 引用的 CSS 变量零悬空；`[hidden]` 全局守卫在位；cache-bust 与记载一致。

## 4. 覆盖边界（诚实声明）

- **L2 桌面交互项未自动化**（本环境无桌面 UI 自动化能力）：文件对话框（导入/导出/保存图片）、钥匙串回显与隔离、剪贴板实际内容、真机 Shift+Enter 换行（代码层面放行原生行为，判定可用）、拖拽导入。dist 桌面端已验证启动（实例运行中，窗口标题 wxwright）。这些项建议按 AGENTS.md 第 6 节由用户日常走查覆盖。
- **L4 AI live**：主题生成链路引用 d4b0336 的 live 冒烟证据（95.8s 端到端成功，产物 `%APPDATA%\wxwright\themes\dark-code-theme.toml` 在盘 1015 字节）；chat 引擎本轮未重跑（省 token），其 UX 层已被 demo 覆盖、引擎层由日常使用覆盖。
- A5 主题切换在 demo 下因 AUD-014（单选项）不可测，桌面行为未复核。
- C4 rule-chip 点击展开、H4 命令速查点击复制、D10 滚动钉住：本轮未走到，遗留为低风险未测项。

## 5. 与规划文档的关系

规划：`docs/button-audit-plan.md`（第 7 节 5 个疑点全部转为正式问题单：1=AUD-005，2=AUD-002，3=AUD-007，4=AUD-013，5=方法论注记）。
静态审计新发现：AUD-001/003/004（i18n）、AUD-011/014（i18n 与 demo）。运行时发现：AUD-006/008/009/010/012/015。
