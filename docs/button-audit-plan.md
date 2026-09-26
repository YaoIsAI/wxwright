# UI 交互面全量审计规划 / Button-by-Button Audit Plan

- 基线：v0.9.0，commit d4b0336，工作树干净
- 制定日期：2026-09-26
- 执行者：任意接管 Agent 或人工
- 状态：待执行
- 结论产出：`docs/button-audit-findings.md`（问题台账，每条带证据）

与 `docs/manual-test-matrix.md` 的关系：那份是主题/视图级的季度回归（渲染质量）；
本文是**交互面全量审计**（每个控件的功能正确性），大版本发布前或连续多次功能迭代后复跑。
两份文档互补，不互相替代。

---

## 1. 目标与验收标准

对 GUI 全部交互控件（按钮 / 自定义下拉 / 输入框 / 拖拽区 / 快捷键 / 宠物交互 / 弹窗开关）
逐一验证「点击后发生了文档声称的行为」，找出所有潜在问题。

验收标准：
1. 第 5 节清单每一行都有结论（通过 / 问题单号 / 豁免理由）。
2. 问题台账每条含：严重度、复现步骤、预期 vs 实际、证据（截图 / DOM 查询 / 测试输出）。
3. 无未处理的 P0 / P1；P2 / P3 要么修复要么明确豁免并写明原因。
4. 修复动作必须走 AGENTS.md 第 9 节交付清单（cache-bust、i18n 双语、dist 同步、测试全绿）。

## 2. 交互面盘点（总口径）

| 口径 | 数量 | 来源 |
|---|---|---|
| `<button>` 元素 | 93 | index.html |
| id 元素 | 194 | index.html |
| addEventListener 绑定 | 118（app.js）+ pet 2 + ai-jobs 1 | 全部 JS |
| 弹窗（modal-overlay） | 9 | settings / agent / theme / poster / prompt / assets / comfy / qr / fit / svgkit（svgkit 与 fit 无 close 按钮的见 P 区） |
| 平台预览壳 | 7（wechat / xhs / zhihu / facebook / instagram / x / linkedin） | app.js SHELL_BUILDERS |
| Tauri 命令（generate_handler 注册） | 约 50 | gui/src-tauri/src/lib.rs |

分区编号（第 5 节使用）：A 顶栏 / B 文章库 / C 编辑器 / D AI 抽屉 / E 预览区 /
F 状态栏 / G 设置弹窗 / H Agent 弹窗 / I AI 主题弹窗 / J 海报工坊 / K Prompt 弹窗 /
L 素材库 / M AI 绘图 / N 尺寸工坊 / O SVG 组件库 / P 弹窗通用。

## 3. 分层验证方法

| 层 | 手段 | 覆盖什么 | 限制 |
|---|---|---|---|
| L0 静态审计 | 脚本化交叉核对（第 4 节命令，可复跑） | 死按钮、孤儿引用、重复 id、命令漏注册、i18n 缺 key、cache-bust | 发现不了运行时行为错误 |
| L1 浏览器 demo | `python -m http.server 8742 -d gui/ui` + 截图 + 交互走查 | 布局、菜单开合、文案、i18n、纯前端状态机 | 无 Tauri 后端：invoke 类功能提示「演示模式不可用」属预期（坑 8）；demo 回退分支同样要测（坑 22） |
| L2 桌面端 | `dist/wxwright-gui.exe` 实点走查 | 真实 invoke、文件对话框、钥匙串、剪贴板、文件夹打开 | 需要先确认 dist 与最新源码同步（铁律：忘同步 = 白测） |
| L3 后端命令 | CLI 等价命令 + `cargo test` | 每个 Tauri 命令的引擎逻辑 | 命令层 Windows 下不可直跑 mock_app（坑 24），走纯函数入口 |
| L4 AI live | 配好 Provider 的真实端到端 | theme / svg / poster / image / chat 生成链路 | 花 token；密钥文件不入库（安全约束） |

证据要求：每个「不通过」结论必须有截图或 DOM/测试输出；「应该好了」不接受（坑 7）。

demo / 桌面边界速查（L1 里预期失败的，L2 必须复验）：
文章库持久化、复制富文本、导出、校验、AI 全家族（chat / theme / svg / poster / image / cloud）、
附件、ComfyUI、公众号绑定、MCP 写入、打开文件夹 —— 均依赖后端。

## 4. L0 静态审计脚本（本次已跑，战果见第 7 节）

```bash
# 死按钮与孤儿引用（注意：动态创建的 id 会误报，如 ai-jump，需人工甄别）
grep -o '\$("[^"]*")' gui/ui/app.js | sort -u | sed 's/\$("//;s/")//' > /tmp/refs.txt
grep -o 'id="[^"]*"' gui/ui/index.html | sed 's/id="//;s/"//' | sort -u > /tmp/defs.txt
comm -23 /tmp/defs.txt /tmp/refs.txt   # 定义了但 app.js 从未引用
comm -13 /tmp/defs.txt /tmp/refs.txt   # 引用了但 HTML 没有
```

```bash
# 重复 id（getElementById 只认第一个，后者成死元素）
grep -o 'id="[^"]*"' gui/ui/index.html | sort | uniq -d
```

```bash
# 事件绑定 vs 控件清单
grep -n 'addEventListener' gui/ui/*.js
```

```bash
# 命令注册完整性（commands.rs 定义 vs lib.rs 注册清单做差集）
grep -o '[a-z_]*,' gui/src-tauri/src/lib.rs | sort -u
grep -n 'pub async fn\|pub fn' gui/src-tauri/src/commands.rs
```

复跑时机：任何前端改动之后、审计执行之前。

## 5. 逐项测试清单

图例：层 = 该项的最低验证层（L1 demo 可验的也要在 L2 复验后端路径）；
优先级 P0 = 主链路损坏即事故，P1 = 功能错误，P2 = 次要功能 / UX，P3 = 打磨 / 彩蛋。

### A. 顶栏（15 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| A1 | logo-btn | 1.5 秒内连点 3 次触发墨仔派对 + toast 彩蛋文案（中英双语各验一次） | L1 | P3 |
| A2 | btn-library | 侧栏展开 / 收起切换 | L1 | P2 |
| A3 | platform-trigger | 自定义下拉开合；再点外部关闭；trigger 显示当前平台 logo 与名称 | L1 | P0 |
| A4 | platform-menu x7 | 7 个平台全部可选；切换后预览壳 / 规则表 / 复制导出物三者联动（demo 回退分支同样可达，坑 22） | L1+L2 | P0 |
| A5 | theme-select | 主题切换立即重渲染预览；重启后记忆选择 | L1+L2 | P0 |
| A6 | btn-theme-ai | 打开 modal-theme，焦点进入描述框 | L1 | P1 |
| A7 | btn-export | 桌面弹出保存对话框，写出 HTML 文件并 toast；demo 提示不可用 | L2 | P0 |
| A8 | btn-poster | 打开海报工坊，预置当前平台尺寸选项 | L1 | P1 |
| A9 | btn-comfy | 打开 AI 绘图弹窗，加载 ComfyUI / 云端模型配置 | L1+L2 | P1 |
| A10 | btn-svgkit | 打开 SVG 组件库并渲染 6 组件列表 | L1 | P1 |
| A11 | btn-assets | 打开素材库，网格显示已有素材与计数 | L1+L2 | P1 |
| A12 | btn-copy | wechat：富文本进剪贴板（粘贴到公众号编辑器验证）；非 wechat：导出对应平台的文案 / Markdown；demo 提示不可用 | L2 | P0 |
| A13 | btn-ai | AI 抽屉开合；再点收起 | L1 | P1 |
| A14 | btn-agent | 打开 Agent 弹窗并加载接手卡 | L1 | P2 |
| A15 | btn-lang | zh / en 全量切换且持久化；所有动态面（含下拉菜单、宠物气泡）重渲染；无 undefined | L1 | P1 |
| A16 | btn-settings | 打开设置，默认 Providers 页，加载 provider 列表 | L1+L2 | P1 |

### B. 文章库（5 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| B1 | btn-import | 多选 .md 导入；frontmatter（title / theme / platform）正确解析入库 | L2 | P0 |
| B2 | btn-new-article | 新建草稿进编辑器，列表出现并选中 | L2 | P0 |
| B3 | library-search | 输入即时过滤列表；清空恢复 | L1+L2 | P2 |
| B4 | article-list 行 | 单击加载文章到编辑器；行内动作（重命名 / 删除）生效；删除有确认且文件真删 | L2 | P0 |
| B5 | library-path | 显示真实文章库路径 | L2 | P3 |

### C. 编辑器（6 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| C1 | editor 输入 | 字数 / 词数 / 图片数实时更新；宠物被打字唤醒；自动保存标记变化 | L1 | P1 |
| C2 | editor 粘贴图片 | 图片自动入库并替换为引用 | L2 | P1 |
| C3 | btn-validate | 按当前平台规则表校验；违规列表面板展示；绿色徽标 / 计数正确 | L1+L2 | P0 |
| C4 | rule-chip | 点击展开当前平台规则说明 | L1 | P2 |
| C5 | violations-panel | 条目文案本地化（空态给文案，绝不 undefined） | L1 | P2 |
| C6 | btn-save / Ctrl+S | 保存成功 toast + 列表刷新；Ctrl+S 等效 | L2 | P0 |

### D. AI 抽屉（11 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| D1 | btn-ai-clear | 清空对话（有确认或可接受无确认） | L1+L2 | P2 |
| D2 | btn-ai-close | 关闭抽屉 | L1 | P2 |
| D3 | 快捷 chip x5（润色/续写/标题/提纲/头图文案） | 各自注入预设 prompt 并发送 | L4 | P1 |
| D4 | btn-ai-attach | 文件选择（PDF/DOCX/HTML/TXT/MD）提取文本入对话；图片走视觉模型 | L2+L4 | P1 |
| D5 | attach-row 移除 | 附件 chip 可删除，发送时不再携带 | L2 | P2 |
| D6 | ai-model-select | 自定义模型菜单开合；切换后 chip 文案更新；点外部关闭 | L1 | P1 |
| D7 | btn-ai-send | 发送消息；生成中按钮变「停止」（按钮态复用，禁止独立停止按钮，见 AGENTS 第 5 节）；停止真的断流 | L4 | P0 |
| D8 | ai-input Enter / Shift+Enter | Enter 发送，Shift+Enter 换行；输入框自适应高度 | L1 | P1 |
| D9 | ai-jump chip | 滚离底部时出现，点击回底部（注意：此元素为运行时动态创建） | L1 | P3 |
| D10 | ai-messages 滚动 | 流式输出时跟随；用户上滚则钉住不抢滚动 | L1 | P2 |
| D11 | 无 Provider 时发送 | 给出可理解错误提示，不静默、不崩 | L1+L2 | P1 |

### E. 预览区（4 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| E1 | seg iPhone15Pro / Pixel8 | 真机切换：边框、按键、岛 / 开孔、home 条全套切换 | L1 | P1 |
| E2 | btn-device-dark | 深浅色切换且壳内配色跟随 | L1 | P1 |
| E3 | splitter | 拖拽调宽顺畅；释放后布局不残破；极端宽度不破 | L1 | P2 |
| E4 | preview iframe（7 壳） | 每个平台壳结构像素级符合设计（wechat 方言 / xhs 笔记 / zhihu 文章页 / fb 卡片 / ig 帖子 / X 帖子 / linkedin 卡片）；壳内 34px 顶部留白与样机状态栏不叠加（坑 21） | L1 | P0 |

### F. 状态栏（3 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| F1 | pet 墨仔 | 单击摸摸（红心 / 语录气泡）；双击换形态（直接切换，禁止加过渡，坑 18）；45 秒入睡；idle 小剧场轮播 | L1 | P2 |
| F2 | malong-btn | 打开二维码弹窗，图片正常显示 | L1 | P3 |
| F3 | link-github | 桌面用 opener 打开浏览器；浏览器 demo 新标签打开 | L1+L2 | P3 |

### G. 设置弹窗（14 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| G1 | settings-nav x4 | 四页切换，active 态正确 | L1 | P2 |
| G2 | provider-list 行 | 点击载入表单编辑；设为 Active 立即生效（AI 面板 chip 更新）；删除有确认；Key 不回显明文 | L2+L4 | P0 |
| G3 | pf-vendor 预设 | 选择厂商自动填 Base URL / 模型名 / 图标 | L1 | P1 |
| G4 | provider-form 保存 | 名称 / 模型 / Base URL / Key 校验；Key 进系统钥匙串（失败回退 %APPDATA% 且不入库、不进日志，安全约束）；新 Provider 出现在列表与 AI 模型菜单 | L2+L4 | P0 |
| G5 | pf-logo-pick / pf-logo-clear | 上传自定义图标立即显示；清除恢复内置 | L2 | P2 |
| G6 | pf-test | 真实连通性测试，成功 / 失败文案清晰；测试中按钮有态 | L4 | P1 |
| G7 | pf-cancel | 丢弃编辑恢复原值 | L1 | P2 |
| G8 | freetokens-link | 打开 free-tokens.org | L2 | P3 |
| G9 | wx-form 绑定 | AppID/Secret 入钥匙串；状态 chip 变已绑定；错误（假凭据）提示明确 | L2 | P1 |
| G10 | wx-unbind | 解除绑定，状态复位 | L2 | P1 |
| G11 | comfy-url / model / comfy-save | 保存配置；comfy_status 回显 | L2 | P1 |
| G12 | comfy-start（设置页） | 一键启动 ComfyUI 进程；状态轮询刷新 | L2 | P1 |
| G13 | comfy-launch-path（设置页输入框） | 填入路径被 G11/G12 正确读取（对照疑点 2） | L2 | P1 |
| G14 | pane-about | 版本号与实际一致 | L1 | P3 |

### H. Agent 弹窗（12 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| H1 | mcp-btn x4（Claude/Cursor/VSCode/OpenCode） | 对应客户端配置真实写入；结果面板给路径与说明；失败给原因 | L2 | P1 |
| H2 | btn-copy-card | 接手卡全文进剪贴板（粘贴验证内容完整） | L2 | P1 |
| H3 | btn-toggle-card | 预览展开 / 收起；内容与 CLI `wxwright agent-card --md` 一致（7 工具面漂移锁定，铁律 9） | L1+L2 | P1 |
| H4 | cli-row x6 | 点击复制对应命令 | L1+L2 | P2 |

### I. AI 主题弹窗（6 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| I1 | theme-preset chip x4 | 点击填充对应描述文本 | L1 | P2 |
| I2 | theme-desc | 手输描述可用 | L1 | P2 |
| I3 | btn-theme-generate | 生成中按钮变停止；成功后新主题立即出现在 theme-select 并应用；TOML 落盘用户主题目录；失败把原因给全（预算阶梯重试链路） | L4 | P0 |
| I4 | 生成中断（点停止） | 协作取消，UI 复位可再生成 | L4 | P1 |
| I5 | 无 Provider 生成 | 提示先配置 Provider | L1+L2 | P1 |
| I6 | 生成结果合规 | 产物过 validate（font-family / 对比度），违规被拒并回喂重试 | L4 | P1 |

### J. 海报工坊（11 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| J1 | poster-preset | 按当前平台填充预设尺寸；选「自定义」显示宽高输入 | L1 | P1 |
| J2 | tpl-chip x3（头图/金句/图文） | 切换模板并重置画布，dirty 标记正确 | L1 | P1 |
| J3 | poster-w / poster-h | 自定义尺寸生效且限制 100-4096 | L1 | P2 |
| J4 | poster-desc + btn-poster-ai | AI 生成海报 HTML；生成中按钮变停止；产物过安全校验（禁外链 / 脚本） | L4 | P1 |
| J5 | poster-html 编辑 | 右侧预览实时刷新 | L1 | P1 |
| J6 | btn-poster-reset | 重置当前模板 | L1 | P2 |
| J7 | btn-poster-save | 保存 PNG 到用户选的位置（本地光栅化） | L2 | P1 |
| J8 | btn-poster-imgset | 图组导出（显示条件：符合出现场景时才显示） | L2 | P2 |
| J9 | btn-poster-insert | 导出并插入文章正文（光标位置正确、预览刷新） | L2 | P1 |
| J10 | 无 Provider AI 生成 | 明确提示 | L1+L2 | P2 |
| J11 | 含外链的 HTML 提交 | 被安全校验拒绝并说明原因 | L2 | P1 |

### K. Prompt 弹窗（3 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| K1 | prompt-ok | 返回输入值（空值返回 null 而不是空串） | L2 | P2 |
| K2 | prompt-cancel / Escape | 返回 null，且多次开关后**监听器不累积**（已核实 done() 有 removeEventListener，复验即可） | L1 | P2 |
| K3 | 连续两次重命名 | 第二次结果正确（防监听器叠加回归） | L2 | P2 |

### L. 素材库（5 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| L1 | assets-open-folder | 打开真实素材目录 | L2 | P1 |
| L2 | 素材项点击 / 插入按钮 | 插入文章 Markdown | L2 | P1 |
| L3 | fit 按钮 | 打开尺寸工坊并载入该图 | L2 | P1 |
| L4 | del 按钮 | 删除有确认，缩略图与文件都消失 | L2 | P1 |
| L5 | 空态引导 | 空库时给引导（含跳转海报工坊的动态按钮） | L1 | P3 |

### M. AI 绘图弹窗（11 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| M1 | imgsrc seg（ComfyUI / 云端） | 切换显示对应区块 | L1 | P1 |
| M2 | comfy-cloud-model + 保存模型 | 保存后云端生成可用 | L2+L4 | P1 |
| M3 | imggen-comfy-start | 一键启动（注意：读取的是设置页的 launch-path，弹窗内输入框疑为死控件，对照疑点 2） | L2 | P1 |
| M4 | comfy-size / steps | 参数传入生成请求 | L2 | P2 |
| M5 | comfy-prompt / negative | 提示词生效；留空用默认反向词 | L2+L4 | P1 |
| M6 | t2i / i2i chips | i2i 显示源图行与 Denoise | L1 | P2 |
| M7 | comfy-pick | 打开素材库选择模式，回填源图 | L2 | P1 |
| M8 | comfy-generate | ComfyUI 离线给清晰提示；在线出图进素材库；生成中有进度与按钮态 | L2+L4 | P1 |
| M9 | comfy-results | 结果图可插入 / 打开工坊 | L2 | P2 |
| M10 | 云端生成（无本地 ComfyUI 时） | 走云端图像 API，产物入库 | L4 | P1 |
| M11 | 生成中断 | 点停止真取消（ComfyUI /interrupt） | L4 | P1 |

### N. 尺寸工坊（7 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| N1 | fit-preset / fit-mode / fit-scale | 画布实时重绘；contain 显示补白行 | L1 | P1 |
| N2 | fit-bg 颜色选择器 + 4 个预设 chip | 补白背景即时生效 | L1 | P2 |
| N3 | fit-export | 导出自动入素材库并插入文章；2x 导出尺寸翻倍 | L2 | P1 |
| N4 | 超大图性能 | 大图导出不卡死主线程 | L2 | P2 |

### O. SVG 组件库（6 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| O1 | svgkit-list 6 内置组件 | 选中后预览与参数面板刷新 | L1 | P1 |
| O2 | svgkit-params 输入 | 改文字 / 颜色实时反映到预览 | L1 | P1 |
| O3 | svgkit-insert | 以文档末尾独立块插入（铁律 8，严禁光标处插入）；插入后引擎复检合规 | L2 | P0 |
| O4 | svgkit-desc + svgkit-ai-generate | AI 生成新组件入列表（带 AI 徽标、可删除）；按钮态停止可用 | L4 | P1 |
| O5 | AI 组件删除 | 列表与持久化同步删除 | L2 | P2 |
| O6 | svgkit-desc 状态 span | 对照疑点 1（重复 id，疑似死元素永不显示内容） | L1 | P2 |

### P. 弹窗通用（3 项）

| 编号 | 控件 | 预期行为 | 层 | 优先级 |
|---|---|---|---|---|
| P1 | modal-close x9 | 全部正确关闭对应弹窗 | L1 | P2 |
| P2 | 遮罩点击关闭 | mousedown 在遮罩上关闭；在弹窗内拖出到遮罩不误关 | L1 | P2 |
| P3 | Escape 键 | 现状：仅 prompt 输入框内 Escape 生效，其余弹窗无 Escape 关闭（对照疑点 3，确认是否补齐） | L1 | P3 |

## 6. 横切回归矩阵（全清单跑完后再过一遍）

| 维度 | 要点 |
|---|---|
| i18n | 每个分区在 zh / en 下各走一遍；重点看动态渲染面（平台菜单 / 模型菜单 / 违规列表 / 壳文案）；缺 key 必须显示 key 名而非 undefined（铁律 4） |
| 深浅色 | 设备深浅色 x 各平台壳组合不出现不可读文本 |
| 键盘 | Ctrl+S；AI 输入 Enter / Shift+Enter；prompt Enter / Escape；Tab 焦点顺序无死循环 |
| 窗口 | resize 后样机缩放、海报预览、splitter 不破 |
| 空态 | 无文章 / 无素材 / 无 Provider / 无网络 四种空态文案完整 |
| 错误态 | 每个 AI 出口断网 / 假 Key / 空 return（length 截断）三类输入的错误文案可理解 |
| 并发与取消 | 生成中关弹窗、生成中切平台、双击提交、任务中再点任务（按钮态模式防重入） |
| 持久化 | Provider / 主题选择 / 语言 / 平台选择 / ComfyUI 配置 重启后保持 |
| 安全 | API Key 不出现在：日志、导出文件、设置回显、剪贴板历史之外（安全约束：agnes ai.txt 隔离不入库） |

## 7. 疑点台账（L0 静态审计已捕获，L1/L2 复核后转正式问题单）

| 编号 | 疑点 | 位置 | 初判 |
|---|---|---|---|
| 1 | `id="svgkit-desc"` 重复：input（SVG 组件 AI 描述框）与 span（组件详情状态栏）同 id | index.html 789 与 802 | `$("svgkit-desc")` 永远命中前者；后者无任何写入方，疑似死显示元素 |
| 2 | `id="comfy-launch-path"` 重复：设置页与 AI 绘图弹窗各一个输入框 | index.html 435 与 657 | 读取方（comfySave/comfyStart）与回填方全部命中设置页输入框；弹窗内输入框为死控件——用户在弹窗里改路径不生效 |
| 3 | Escape 不能关闭除 prompt 外的 8 个弹窗 | app.js（全局 keydown 仅 Ctrl+S） | 一致性缺口，确认是否补齐 |
| 4 | `prompt-ok/cancel` 存在两个空绑定 `() => {}` | app.js 3290-3291 | 无功能危害（真绑定在 uiPrompt 内且正确移除），属噪音代码，建议清理 |
| 5 | 方法论注记：`ai-jump` 引用告警为假阳性（app.js 2887 运行时动态创建） | app.js 2887-2895 | 复跑 L0 时对「引用缺失」类结果先查动态创建，避免误报 |

## 8. 执行顺序与交付物

1. 阶段 0（L0）：复跑第 4 节脚本，更新第 7 节台账。
2. 阶段 1（L1）：起 8742 demo，按 A-P 顺序走查并截图；demo 不可用的项标记「待 L2」。
3. 阶段 2（L2）：确认 dist 与源码同步后桌面走查所有 L2 项；文件对话框 / 钥匙串 / 剪贴板逐项证据。
4. 阶段 3（L4）：配置 Provider 后跑 AI 全家族（theme / chat / svg / poster / image / cloud / test 连接）；预算与取消路径必测。
5. 阶段 4：汇总 `docs/button-audit-findings.md`（按下面模板）；修复排期；修复走 AGENTS.md 第 9 节交付清单。

问题台账模板：

| ID | 区域 | 严重度 | 复现步骤 | 预期 | 实际 | 证据 | 状态 |
|---|---|---|---|---|---|---|---|
| AUD-001 | A4 | P1 | ... | ... | ... | 截图/DOM | open / fixed / wontfix(理由) |

严重度定义：P0 主链路损坏或数据丢失；P1 功能错误但可绕过；P2 次要功能 / UX 缺陷；P3 打磨项。

## 9. 关联坑位（执行时对照 AGENTS.md 第 7 节）

| 审计场景 | 相关坑 |
|---|---|
| L1 demo 走查 | 坑 8（demo 无后端属预期）、坑 22（demo 分支也要接渠道壳）、坑 14（后台 tab 节流） |
| 壳与样机 | 坑 21（假状态栏叠加）、坑 20（自定义下拉） |
| 修复阶段 | 坑 6（cache-bust）、坑 4（命令漏注册）、坑 2/3（重复定义与裁尾）、坑 11（CSS 变量名）、坑 12（hidden 被 flex 覆盖） |
| AI live | 坑 24（Windows mock_app 不可跑，走纯函数入口）、坑 16（密钥隔离复查） |
