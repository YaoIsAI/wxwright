# PRD：wxwright —— Rust 多端微信公众号文章编辑器
### CLI / MCP 原生，AI Agent 一键发文，复制粘贴零样式失真

> 版本：v1.5（实现同步版）｜ 日期：2026-09-25 ｜ 状态：v1.0 已实现交付，本版记录 GUI 增量与扩展方向
> v1.5 变更：新增 §15 已交付能力对照（桌面客户端全部 GUI 增量）与 §16 海外社交媒体扩展方向（Facebook/X/Instagram 等，v2 评估）；§3.8 设计系统在客户端全面落地；写作宠物、SVG 组件库、海报工坊、尺寸工坊、ComfyUI/云端双绘图源等以「已交付」标注回填 §4。
> v1.4 变更：**项目定名 `wxwright`**（原 WeDraft 因同品类撞名弃用，全渠道查重与决议见 §11）；§1.2 差异化声明据实修正（新确认同类开源项目）；全文标识符/命令名同步。
> v1.3 变更：**架构方向修正——本地极致性能 × 极简架构**。§3.1/§3.2 撤回 v1.2 的 daemon/JSON-RPC 单机服务拓扑，改为**单二进制、单进程、零常驻服务**；微服务式的边界纪律仅保留在编译期（trait + 版本化 DTO）。§1.5 非目标、§5.7 架构前提、§10 里程碑、§14 开放问题同步。
> v1.2（保留）：§3.8 设计系统（大厂极简 · 全 SVG · 产品面零 emoji）、§5.7 极致性能 SLO 与 CI 门禁、§6 agent-card / mcp install、§9-7/8/9 验收。
> 拟开源仓库：GitHub `wxwright`（命名已定稿，查重矩阵见 §11）
> 本文档面向人类开发者与 AI Agent 双重读者：Agent 实现时请按 §3 架构分层与 §5.3 规则表逐条落地，验收以 §9 为准。

---

## 1. 产品概述

### 1.1 一句话定位
任何 AI Agent（或人类）把 Markdown 交给 wxwright，一条命令/一次 MCP 调用，就能得到**符合微信公众平台编辑器开发规范**的富文本，一键复制到公众号后台**零样式失真**，或直接通过公众号 API 发布草稿。

### 1.2 背景与问题
- 公众号编辑器是自研 ProseMirror 变体，对 HTML/CSS 有强约束：外部样式表被丢弃、class/id 失效、大量第三方排版工具的"魔法样式"（opacity 藏图、固定像素宽、height:0 折叠）在编辑器里正常、**发布后/移动端上直接坏掉**；Dark Mode 下渐变背景还会被算法转纯色。
- AI Agent 生成的文章普遍是 Markdown/自由 HTML，直接粘贴进公众号必然出现：样式丢失、图片失效（外链被拦截）、文字重叠、移动端截断、图片无法在后台二次编辑等问题。
- 同赛道现状（2026-09 查重确认）：135/秀米类面向人类点击操作；markdown-wechat 系为脚本工具；GitHub 已有 `pafa/WeDraft`（TypeScript + Tauri 的公众号排版工具，亦宣称 CLI/MCP/Web/macOS，2026-09 创建、活跃）。但**尚无"官方规范全量规则引擎（生成→归一化→校验三层完整覆盖 §5.3）、单二进制极致性能（§5.7 SLO：TS/Electron 形态做不到的 8ms 冷启动与 8MB 分发）、官方 verify CLI 对齐 + 四视图真值验证"的开源方案**——这就是 wxwright 的差异化：不是又一个编辑器，而是 **Agent 时代的公众号排版基础设施**。

### 1.3 目标用户与核心场景
| 用户 | 场景 |
|---|---|
| AI Agent（Claude/千问/Cursor/自建流水线） | 通过 CLI 或本地 MCP server 调用：生成 Markdown → 转换 → 校验 → 上传素材 → 写入剪贴板 / 创建公众号草稿，全程无人值守 |
| 内容运营/自媒体 | 本地 GUI 编辑预览、挑主题模板、一键复制粘贴进公众号 |
| 技术写作者 | 用 Markdown 写代码块/公式/表格，担心公众号渲染破坏排版的交给引擎规范化 |
| 二次开发者 | fork 模板、注册自定义组件、嵌入自家系统（引擎是纯 Rust crate，可被 GUI/CLI/server/移动端共享） |

### 1.4 北极星指标
**Agent 发文一次通过率**：AI Agent 从提交 Markdown 到"粘贴进公众号后人工检查无样式问题"的一次成功比例，目标 ≥ 95%（以 §9 验收用例集度量）。

### 1.5 非目标（v1 明确不做）
- 不做小程序/视频号/第三方平台图文（仅公众号 MP 图文消息）；
- 不做图文混排的所见即所得"拖拽设计器"（排版能力=官方规范内的确定性映射，不做规范外魔法效果）；
- v1 不做移动端 App，但架构必须为其预留（§3.6）；
- 不做云端托管服务（本地优先、数据不出机）；
- 不做 daemon / 常驻后台 / 开机自启 / 托盘服务（极简架构：装完即用、退出无痕，§3.2）。

---

## 2. 术语与关键事实（实现前必读）

| 术语 | 说明 |
|---|---|
| **官方规范** | 《微信公众平台编辑器开发规范》（本产品约束的唯一权威来源；附件已获取，其 §4.4 SVG 细则尾部被截断，实施前需补全原文） |
| **verify-article-structure-spec** | 官方规范配套开源仓库（`github.com/wechatjs/verify-article-structure-spec`），基于 puppeteer 的真实浏览器校验 CLI，规则以仓库内 `verify_article_structure.md` 为权威定义；退出码 0=通过 / 1=违规 / 2=异常。**本产品的内置校验器与之对齐，并把它作为 CI 真值门禁** |
| **MP 图文** | 公众号"图文消息"文章类型，富文本 content 字段 |
| **mmbiz URL** | 微信素材 CDN（`mmbiz.qpic.cn`），进入该域的图片后台可二次编辑、读者端不拦截 |
| **span[leaf] / section[nodeleaf]** | 公众号编辑器行内容器/特定功能容器标记，见官方规范 §2.2/§2.3 |
| **零失真** | 定义：复制粘贴（或 API 发布）后，编辑器视图、已发布 PC 视图、移动视图、Dark Mode 四态与设计预览一致 |

---

## 3. 总体架构（多端共享引擎）

### 3.1 设计原则
**一个纯 Rust 引擎（`wxwright-core`），三种宿主壳（CLI / MCP server / GUI），一个合规产物契约。** 所有"正确性"逻辑（HTML 规范化、规则引擎、剪贴板载荷构造）只存在于 core，宿主壳不得复刻——这保证未来 Android/iOS 通过 FFI 复用同一颗大脑。**极简架构铁律（v1.3）**：单个静态二进制就是完整产品——无 daemon、无常驻、无自启、无 IPC；能力边界只存在于编译期（trait + 版本化 DTO）。大厂微服务的组织学红利（契约先行、独立可测试、可替换）全部保留在代码结构上，运行期成本为**零**——这是"本地极致性能"的架构前提。

### 3.2 分层与组件图
```
                    ┌──────────────────────────────────────────┐
   人类 ──GUI──►    │  宿主壳层                                 │
   Agent ─CLI─►     │  wxwright-gui(Tauri2)  wxwright-cli  mcp    │
   Agent ─MCP─►     ├──────────────────────────────────────────┤
                    │  wxwright-core（纯 Rust，无 UI/无网络 IO 假设）│
                    │  ├ parser     pulldown-cmark → 内部 IR     │
                    │  ├ ir         语义块(标题/段落/图/表/代码/卡片)│
                    │  ├ template   TOML 主题/组件 + minijinja 渲染│
                    │  ├ normalizer ★ 官方规范规则引擎（§5.3）      │
                    │  ├ validator  无浏览器合规校验（对齐官方 CLI） │
                    │  ├ clipboard  富文本载荷构造（text/html 制品） │
                    │  └ imgpipeline 图片资产抽象（本地/URL/mmbiz） │
                    ├──────────────────────────────────────────┤
                    │  适配器层                                 │
                    │  wechat-mp（reqwest 封 API：token/素材/草稿/发布）│
                    │  sys-clipboard(arboard)  sys-image(image crate)│
                    └──────────────────────────────────────────┘
```
crate 划分（workspace）：`wxwright-core` / `wxwright-cli` / `wxwright-mcp` / `wxwright-gui`(Tauri) / `wxwright-mp`(公众号 API，可选 feature) / `wxwright-ffi`(后期移动端)。

**进程拓扑（单进程 · 零服务）**
- **唯一运行形态**：CLI / MCP = 单个静态二进制，所有能力为进程内函数调用——零序列化、零 IPC、零守护；上传队列、限速重试、钥匙串凭据读取都在进程内同步/异步完成。冷启动 p50≤8ms 与常驻内存 ≤50MB（P-1/P-6）正是以此为前提：启动与内存预算中没有"框架成本"，全部花在计算本身；
- **边界在编译期，不在运行期**：parser / template / normalizer / validator / imgpipeline 各自是独立 trait + 版本化 DTO（schema 单一来源），可独立单测、可独立替换，兼容性矩阵进 CI——但同包进同一进程；
- **唯一出进程例外**：官方 puppeteer 校验桥（`--official-check`，node 子进程）——位于 Rust 信任边界之外且仅 CI 使用，与主链路无关；
- **未来演化（非承诺）**：仅当"并发上传共享/多端凭据缓存"出现真实需求时，评估可选组件形态（§14-6）；默认永远单进程。

### 3.3 技术选型
| 层 | 选择 | 理由 |
|---|---|---|
| GUI | Tauri 2.x（Win/macOS/Linux） | 一份 WebView 壳 + 共享 core；比 Electron 轻；移动端后期可评估复用（Tauri 2 已有移动实验线）或直接 FFI+原生壳 |
| CLI | clap 4 + 子命令 | 面向脚本/管道的稳定接口 |
| MCP | rmcp（官方 Rust SDK） | 标准本地 MCP server，`stdio` 与 `sse` 两种 transport |
| Markdown | pulldown-cmark | 纯 Rust、GFM 扩展全 |
| 模板 | TOML 主题包（零模板引擎） | 主题即数据：色板 + `[block.<role>]` 内联样式字典，渲染期直接展开；第三方零代码扩展，热路径无引擎开销 |
| HTML 处理 | lol_html（流式单遍重写）+ 自研 walker 规则引擎 | 规范化按规则改写 DOM；流式解析无需整树，契合 P-2 预算（scraper/tl 全树方案已弃用） |
| 剪贴板 | arboard 3.6（桌面三平台 `Set::html`） | Windows CF_HTML / macOS public.html / Linux text/html，失败降级纯文本 |
| 图片 | image crate（头流式探测尺寸） | 读真实像素宽→自动补 `data-w`，不解码全图 |
| 校验 | 内置 validator + **官方 puppeteer CLI 作为 CI 真值** | 内置快、官方准；两者对齐是本 PRD 的工程承诺 |
| i18n | core 内嵌 en/zh-CN 双列 catalog（`rules.rs` / `i18n.rs` 同风格） + 前端 JS 语言包 | 比独立 TOML 语言包更简单，key-parity 由测试锁定（§9-6）；规则表双语天然随 RULES 单源生成 |
| 并发 | std::thread + blocking ureq（无 tokio/rayon） | CLI 依赖面最小化（P-1/P-7）；文章级批量顺序处理足够快；GUI 异步用线程池 |
| 增量 | 220ms debounce 全量重渲（sub-ms 转换） | GUI 打字流畅由引擎速度保证，不引入 notify/memoize 复杂度 |
| 性能基准 | `wxwright bench`（p50/p95，--json CI 友好）+ CI 记录 | 自测命令进 doctor 生态，回归 >5% 附 diff（§5.7-C） |

### 3.4 核心数据流（Agent 一键发文链路）
```
Agent 提交 Markdown（CLI stdin / MCP 参数）
  → ① parser → IR（结构化的语义块，携带 role/style token）
  → ② template 渲染 → 公众号方言 HTML（已按官方结构生成：section 块级 + span[leaf] 行内，全内联样式）
  → ③ imgpipeline：逐图落地 —— 本地文件/HTTP 拉取 →（配了 API 凭据）上传素材拿 mmbiz URL 并注入 data-w/data-ratio；未配则保留原样并输出警告清单
  → ④ normalizer：规则引擎扫描修正（§5.3 全表），产出「修正报告」
  → ⑤ validator：内置合规校验；--strict 时以退出码阻断（并把 HTML 交给官方 puppeteer CLI 复核，CI 模式）
  → ⑥ 出口三选一：
     a) clipboard：构造 text/html 制品写入系统剪贴板 → 用户/自动化在公众号编辑器粘贴
     b) draft：调 草稿箱 API 直接创建公众号草稿（图片已走 a 中 mmbiz 化，最稳）
     c) file：导出 .html / .md+manifest.zip 供离线/审计
  → ⑦ 输出 JSON 结果（产物 id、警告、修正记录、校验结论）——供 Agent 决策下一步
```

### 3.5 "零失真"的成立条件（技术命题拆解）
复制到公众号不失真 = 同时满足四条，缺一不可，全部由引擎保证：
1. **载荷正确**：剪贴板里的 `text/html` 制品自包含——所有 CSS 内联、无 `<style>`/class/id/script、无外部字体/图标引用（公众号粘贴时只保 inline style）；
2. **结构正确**：产出 HTML 本身就长在官方规范白名单内（模板渲染阶段就按规范生成，normalizer 是第二道防线而非唯一防线——"先生成正确的，再校验"）；
3. **图片正确**：全部为 mmbiz 域（或明确接受外链风险的降级模式），且带 `data-w`；
4. **真值验证**：以官方 verify CLI + 四视图（编辑器/已发布 PC/移动/Dark Mode）目检用例为发布门禁。

### 3.6 移动端预留（Android/iOS，v2+）
`wxwright-ffi` 暴露 C ABI：`wxw_convert(md, theme) -> html`、`wxw_validate(html) -> report`、`wxw_build_clipboard_payload(html) -> bytes`。Android 用 NDK/JNI、iOS 用 XCFramework 封装；移动端场景是"编辑/预览/生成分享文件"，**写入他 App 剪贴板受系统限制，剪贴板复制能力仅桌面端提供**。

### 3.7 国际化与多语言（i18n / l10n）
> 先回答前置问题：**多端编译 = Rust 引擎原生跨端**——`wxwright-core` 无平台 UI/网络 IO 假设，为 Windows / macOS / Linux（x86_64 + aarch64）编译，移动端经 FFI 复用同一引擎（§3.6）。本节处理正交的第二维度：**多语言是一等架构需求，不是事后补丁**。

**A. 四个需要本地化的文本面**
| 面 | 内容 | 机制 |
|---|---|---|
| ① CLI 人类输出 | help、错误、validate/fix 报告、doctor 清单 | rust-i18n + TOML 语言包 |
| ② GUI UI | 菜单、设置、按钮、预览标签 | Tauri 前端 i18next + JSON 语言包 |
| ③ 校验报告 | §5.3 每条规则的说明文字 + 修复建议（rule_id 保持英文，描述本地化） | 与 ① 同一 catalog |
| ④ 文档/README | 仓库主文档、docs 站、错误码手册 | 英文主文档 + 本地化版本（§12.1） |

**B. 机器面永不本地化（硬契约）**
- `--json` 输出、MCP `structuredContent`、退出码、规则 ID（R-1.3…）、主题 key：**恒为英文常量**——Agent 依赖的机读面必须跨语言稳定，否则"多语言"会杀死 Agent 接入方。
- 只有"给人读"的文本走本地化；同一数据流可双语，但 key 不变。

**C. 语言选择与回退**
- CLI 优先级：`--lang` > `WXWRIGHT_LANG` > `LC_ALL/LANG`（Unix）/ Windows 用户 locale > `en` 兜底；
- GUI：首启跟随系统语言，设置持久化覆盖，应用内切换即时生效；
- 缺失 key 回退英文而非失败；CI key-parity 门禁保证不漂移（§9-6）。

**D. 首发语言与扩展流程**
- v1.0 必须双语：**English + 简体中文**（源语言英文，zh-CN 为第一翻译）；zh-TW / ja / ko / de / fr / es / ru / pt-BR 框架就绪后由社区驱动（`i18n` good-first-issue 标签，Crowdin/Weblate 可选）；
- RTL（阿拉伯语等）：v1 明确不支持并登记为 known limitation；v2 评估 GUI CSS `dir` 与 CLI 双向文本。

**E. 平台术语例外（微信生态特有，容易翻错）**
微信公众平台后台、接口与帮助文档均为纯中文。"草稿箱 / 素材库 / 群发 / 订阅号"等专有名词，在非中文 UI 中按"**中文原文 + 括注翻译**"呈现（如 `草稿箱 (Drafts)`）——否则全球用户在中文后台找不到对应入口。i18n 层区分"UI 语言"与"平台术语"：后者不翻译，只注释。

**F. 主题/模板文案解耦**
模板不得内嵌自然语言文案；UI 词条走 `{{t:key}}` 占位，由宿主语言包解析——保证社区主题天然多语言。

### 3.8 设计系统：大厂极简 + 全 SVG 图标（产品硬规则）
**A. 视觉基调（GUI / CLI / 文档面一致）**
- 原则「内容即主角」：参照 Linear / Notion / Vercel / Apple HIG 的克制风格；
- 设计 tokens：中性灰 5 阶 + **单一强调色**（默认钴蓝 `#2F6CEA`）；间距 4pt 网格、圆角 8px；层级仅用边框与留白表达（无阴影堆砌）；浅色/深色双主题全部 token 化；微动效 ≤150ms；
- 字体（仅约束本地 UI；公众号产物仍受 §5.3 R-3.1 零 font-family，二者不冲突）：Inter + Noto Sans SC（开源可分发）、等宽 JetBrains Mono；
- 内置默认主题改版为 **"Minimal"**：内容网站审美——三级灰度字色 + 单一强调色 + 大留白。

**B. 图标体系（只允许 SVG）**
- 全部图标来自 `wxwright-icons`：24×24 viewBox、stroke 1.5、`currentColor` 单色（自动适配深浅色）、打包期内联 sprite 进 Tauri、零外部请求；可基于 Lucide（ISC 许可）二次规整并保留许可证声明；
- 图标语义必须配文字标签，图标永不承载唯一语义（可访问性 + i18n 友好）；
- **emoji 禁用范围**：GUI、CLI 人类输出、MCP 返回文案、README/docs（所有语言版本）、主题与组件模板、错误与校验报告、CHANGELOG——CI 全库扫描（§9-8）；
- CLI 输出符号统一 ASCII 前缀（`OK / WARN / ERR / →`），ANSI 色彩尊重 `NO_COLOR` 与 `--no-color`。

**C. 为什么这么定（全球开源依据）**
① Windows 等平台 emoji 字体渲染分裂；② emoji 跨文化语义歧义；③ 高分屏与打印发虚、不可主题化；④ 读屏工具把 emoji 念成噪音。tokens.json + SVG 库均为单一来源，进 PR 模板审查清单（新 UI 必须用 token、新图标必须进 SVG 库）。

### 3.9 应用图标规范（App Icon，打包默认图标）
> 设计候选母版参考：`design/icon-candidates/`（项目内三份 1024 viewBox 方案：A 波形 W / B 光标纸飞机 / C 对话文档卡），最终定稿走本节规范评审，不视为已锁定。

**A. 设计约束（与 §3.8 同源）**
- 单一概念、纯几何、零文字零 emoji 零渐变零阴影；钴蓝 `#2F6CEA` 为唯一品牌色 + 白色图形（深浅桌面均有对比）；
- 主体图形色数 ≤2；造型在 16px 下仍可读为同一符号（评审硬标准）；
- 语义优先级：排版/写作 × 微信生态的联想（如波形 W 呼应"We"，文档卡呼应"图文"），避免与既有知名 App 图标撞型（发布前做一次视觉查重）。

**B. 构图网格（单一母版制）**
- 唯一事实源 = `assets/icon/master.svg`，viewBox 1024×1024；
- 背景圆角矩形 rx=225（≈ iOS 连续曲率观感；Android adaptive 层除外，见 D）；
- 主体图形安全区：居中 80%（即留 10% 出血边距，约 102px），任何平台蒙版（圆/方/水滴）裁切后主体不碰边；
- 描边类元素线宽 ≥112/1024（≈11%），保证小尺寸笔画不断裂。

**C. 导出管线（打包时机械生成，禁止手改产物）**
- 输入仅母版 SVG：`cargo tauri icon assets/icon/master.svg` 一键生成全平台产物（Tauri 官方工具链，与 §3.3 GUI 选型一致）；
- 产物入库 `src-tauri/icons/`：Windows `icon.ico`（16/24/32/48/64/128/256 全帧）、macOS `icon.icns`（含 @2x）、Linux hicolor PNG 集（16–512）；
- CI 校验：`icons` 产物与母版 hash 一致（母版变而未重导出则阻断）——图标与产物永远同源。

**D. 平台适配细则**
| 平台 | 形态 | 注意 |
|---|---|---|
| Windows | .ico 多帧内嵌进 exe（Tauri bundle 自动） | 16/32 帧单独目检；任务栏/托盘复用同库 |
| macOS | .icns，Dock 大圆角蒙版由系统施加 | 母版已含 rx，不叠加系统观感冲突；配 Dark/Aqua 均验证 |
| Linux | PNG 主题目录 + .desktop 引用 | 发行版图标尺寸惯例 512 兜底 |
| Android（v2 FFI 预留） | Adaptive Icon：前景=白图形（缩至安全 66%）、背景=#2F6CEA 纯色层 | 母版需同时导出"去背景版"作 foreground 源 |
| iOS（v2 预留） | 无圆角原图 1024，系统加蒙版 | 禁止导出带透明圆角的版本（会被二次裁切） |

**E. 验收清单**
① 16/32/64/128/256 五档缩放目检（Windows 资源管理器缩略图视图实测截图入 PR）；② 纯黑白单色版（打印/ favicon 场景）仍可辨识；③ 深色与浅色桌面前后对比截图；④ 与撞型候选（微信、Typora、Notion、Obsidian 图标）并排目检无混淆。

---

## 4. 功能需求（P0 = v1.0 必须）

### 4.1 编辑与转换
| ID | 需求 | 优先级 |
|---|---|---|
| F-01 | Markdown(GFM) → 公众号 HTML：标题、段落、粗斜体、删除线、链接、有序/无序/任务列表、引用、分割线、表格、代码块、图片、公式（转 SVG/图片策略见 §5.4） | P0 |
| F-02 | 主题模板系统：内置 ≥3 套（默认素黑 / 科技蓝 / 杂志衬线），TOML 可自定义；模板 = 各语义块的角色样式表（颜色/字号/间距/卡片），全部样式编译期展开为内联 | P0 |
| F-03 | 组件库：留言卡片、划重点、居中标题、图文笔记容器、提示块（note/warn）、目录卡；**全部在生成阶段通过官方规范自检** | P0 |
| F-04 | 代码块：渲染为高亮 HTML（服务端 syntect 着色 → 全内联 span），**禁用 `<pre>` 包裹正文**（官方规则 1.8），代码用 `<section style="overflow-x:auto">`+等宽内联方案；附"代码图卡"可选模式（代码整块转图片，保真最强但不可选中复制） | P0 |
| F-05 | 表格：单元格 `min-width` 策略 + 总宽 100% 自适应，杜绝固定 px 溢出（官方规则 1.4.2） | P0 |
| F-06 | 一键复制：CLI `wxwright copy` / GUI 按钮 / MCP `wxwright_copy`，写入剪贴板制品并附带操作提示 | P0 |
| F-07 | GUI：实时双栏预览（左编辑右"手机框"渲染，含 Dark Mode 切换预览） | P0 |
| F-08 | 导出：`.html` 自包含文件（供外部工具/存档） | P0 |
| F-09 | 草稿直发：`draft create/update`（走公众号草稿箱 API） | P1 |
| F-10 | 群发发布：`publish`（需服务号/认证资质；二次确认 + 幂等锁，防误发） | P2 |

### 4.2 图片管线（核心体验，常被低估）
| ID | 需求 | 优先级 |
|---|---|---|
| I-01 | 本地图片路径解析 + `image` crate 读真实宽高 → 自动注入 `data-w`、`data-ratio`（官方 1.4.3 检测兜底要求） | P0 |
| I-02 | 上传公众号永久素材（`material/add_material`, type=image）→ 重写 `src` 为 mmbiz URL → 后台可二次编辑图片 | P0 |
| I-03 | 无凭据降级：保留原 URL/内联 base64（大图警告），产出「未 mmbiz 化」清单；**剪贴板模式下图必须已是 mmbiz 或微信可拉取的 https 直链，否则粘贴必挂，CLI 默认阻断并提示** | P0 |
| I-04 | GIF 保留（公众号支持动图，重编码需保帧） | P1 |
| I-05 | 图片批量压缩/格式策略（jpg/png/webp→上传兼容转换） | P1 |

### 4.3 校验与修正（差异化能力）
| ID | 需求 | 优先级 |
|---|---|---|
| V-01 | 内置 validator：§5.3 全规则静态校验，输出人类可读明细 + `--json` 结构化报告（与官方 CLI 输出字段兼容） | P0 |
| V-02 | normalizer：可自动修的规则直接修（补 data-w、行高下限、固定宽度→百分比、拆冗余嵌套、leaf 违规结构重组…），不可修的报告 | P0 |
| V-03 | 官方 CLI 桥接：检测到本机 node/puppeteer 环境时，可调用 `verify-article-structure-spec` 复核（`--official-check`），CI 模板随仓库提供 | P1 |
| V-04 | 四视图回归集（§9）：仓库内置 golden HTML + 人工目检清单 | P0 |

---

## 5. 关键技术设计

### 5.1 渲染目标格式：公众号方言 HTML
- 块级一律 `<section>`；行内文字一律 `<span leaf>` 包裹（无文本的装饰容器不加）；
- **所有样式内联**；不输出 class/id/style 外链/script/iframe/form；
- 不写任何 `font-family`（§5.3 规则 F-1）；
- 段落用 `<p>` 或 `<section>`，普通文本禁 `<pre>`（规则 1.8）；
- 模板引擎输出的就已是方言 HTML——normalizer 定位为"第二道防线 + 用户手改 HTML 的修复器"。

### 5.2 图片 URL 获取的两种合规姿势
| 方式 | 适用 | 风险 |
|---|---|---|
| A. 剪贴板粘贴（v1 主路径） | 无 API 凭据也能用；但 `src` 必须是微信可拉取的 https 直链，推荐仍先走 mmbiz 化（本地 GUI 内建登录态上传通道见 §5.5） | 外链图有加载延迟/防盗链失败 |
| B. 公众号 API 上传素材 → mmbiz → 草稿箱发布（Agent 无人值守主路径） | 全自动、图可后台编辑、无防盗链问题 | 需用户配置 AppID/Secret，且素材上传有接口权限要求（需在 README 权限清单列明） |

### 5.3 官方规范规则表 → 引擎行为映射（实现契约）
> 编号对应官方规范章节；"生成策略"=模板层义务，"归一化"=normalizer 义务，"检测"=validator 义务。

| # | 官方规则（摘要） | 生成策略 | 归一化（可自动修） | 检测（不可修则阻断） |
|---|---|---|---|---|
| R-1.1 | 禁 `img opacity:0` + SVG 背景图叠放技巧（致后台无法改图） | 模板禁用该技巧 | 拆解该模式→还原为常规 `<img>` 并警告 | 发现"opacity:0 图 + 同位 svg bg"即阻断 |
| R-1.2 | 禁 `caret-color` 完全透明 | 模板不输出 | 删除透明 caret-color 声明 | 阻断 |
| R-1.3 | `line-height` 不得小于字号致叠字（纯图容器/单行文本豁免） | 文字容器默认 `line-height ≥ 1.5` 或 ≥ 字号 | 违规时提升至 max(1.5em, 字号px) | 阻断（多行且行高<字号） |
| R-1.4 | 禁固定像素宽度（居中不一致/溢出/比例失配三态检测）；`margin-left` 过大偏移亦违规；豁免 `data-ignore-width` | 模板一律 `width:100%`/百分比/`max-width` | 固定 px 宽度按 677px 设计基准换算为百分比（含上限） | 阻断；`data-ignore-width` 子树豁免 |
| R-1.4w | `<img>` 建议带 `data-w`（原始像素宽），否则加载超时会误报 | 注入 data-w+data-ratio（见 I-01） | 缺失时读图补写 | 警告 |
| R-1.5.1 | 禁含文字容器 `height:0`（移动端正文不可见；SVG 交互容器/无文字豁免） | 模板禁用 | 移除 height:0 并警告 | 阻断（含文字） |
| R-1.5.2 | 禁固定小高度裁剪文字内容（滚动容器豁免） | 模板禁用 | 移除裁剪高度 | 阻断 |
| R-1.6 | `text-align` 禁 `start/end`（iOS 兼容差） | 模板只用 center/left/right/justify | 自动替换（start→left, end→right） | 阻断 |
| R-1.7 | SVG `<animate begin>` 不得只写 `touchstart`（PC 失效） | 模板写 `begin="touchstart; click"` | 补 click | 阻断 |
| R-1.8 | 普通正文禁 `<pre>`（white-space:pre 移动端截断） | 代码块用受控方案（F-04） | `<pre>` 文字段落 → section 化重写 | 阻断 |
| R-2.1 | 同标签+同内联样式+单子节点的嵌套链 ≤10 层（媒体标签不计） | 模板避免深链 | 自动折叠冗余链 | 阻断 |
| R-2.2 | `<span leaf>` 内禁块级元素 | 模板遵守 | 违例块级元素上提/重排结构 | 阻断 |
| R-2.3 | `<section nodeleaf>` 只包官方组件或 `<img>` | 模板遵守 | 违例去 leaf 化 | 阻断 |
| R-3.1 | **不设任何 font-family**（破坏编辑器/移动双端一致性） | 模板零输出 | 剥离全部 font-family 声明（含 !important） | 阻断 |
| R-4.1.1 | 文字/背景对比度过低或过高会被 Dark Mode 算法干预 | 模板色板内置 WCAG 对比度约束 | 报告"将被算法改写"的颜色 | 警告（保留创作自由） |
| R-4.1.2 | 文字下方渐变背景 Dark 下会被 mix 转纯色 | 有文字容器禁用渐变背景 | 渐变→纯色并提示 | 警告 |
| R-4.1.3 | 纯装饰渐变（无文字）可保留 | — | — | — |
| R-4.2 | 背景放公共容器；禁绝对定位破坏视觉/结构顺序（Dark 深度优先遍历） | 模板用容器背景 | 报告错位风险 | 警告 |
| R-4.3 | 图片不承载纯文本；透明图注意与 #191919 底色对比；bg-image 补色机制 | 模板遵守 | — | 警告 |
| R-4.4 | SVG 内容 Dark 不做转换（含截断待补细则） | 模板保守 | — | ⚠️**附件截断项：实施前从官方仓库 `verify_article_structure.md` 补齐规则全集，映射表随之更新** |

> 注：Dark Mode 类规则官方定位是"调校得当体验更优"而非硬违规，引擎对 R-4 系列默认输出警告 + 提供 `--fix-dark` 一键按推荐改法（容器化背景/去文字渐变）；R-1/R-2/R-3 默认阻断。

### 5.4 公式与特殊内容
- LaTeX：主策略 KaTeX 预渲染 → 转 `<img>`（公式图随文上传 mmbiz，规避 SVG 文本 Dark 不转换问题）；备选内联 SVG 模式（标注已知 Dark 限制）。
- 视频/音频/小程序卡片/投票：保留 nodeleaf 容器透传位（模板出"占位卡"，提示用户在公众号后台插入官方组件——API 粘贴通道无法凭空生成合法媒体节点）。

### 5.5 剪贴板工程细节（易踩坑，列为需求）
- `text/html` 制品内容 = 完整方言 HTML（UTF-8，注释声明 `<!-- generated by wxwright -->` 便于排查）；三平台富文本由 arboard 3.6 `Set::html()` 写入（Windows CF_HTML / macOS public.html / Linux text/html），任一平台失败自动降级纯文本并在结果 `html_flavor` 中标明；
- 跨浏览器粘贴差异回归（§9）：Chrome/Edge/Firefox × Win/macOS/Linux；GUI 内提供"复制到剪贴板"按钮 + CLI 同源实现；
- 失败兜底：剪贴板不可用（无显示环境的 CI）时 `--out file.html` 导出 + 打印指引。

### 5.6 凭据与安全
- AppID/Secret 存系统钥匙串（macOS Keychain / Windows Credential Manager / libsecret），配置文件仅存引用；
- 日志与错误信息一律脱敏（access_token 全掩码）；
- CLI 永不静默发布：`publish` 必须显式 `--yes` 且默认仅创建草稿。

### 5.7 极致性能工程（SLO · 手段 · 门禁）
**0. 架构前提**：极简架构（单进程、零 IPC、零序列化、零框架）= 最大性能杠杆（§3.1/§3.2）。下列预算与手段全部以此成立：任何"加一层进程/中间件"的提案，必须先证明其收益大于它的启动与内存税。

**A. SLO 预算**（release 构建，CI 固定 runner 实测，本地 `wxwright --benchmark` 可复现）

| # | 指标 | p50 | p95 | 测量 |
|---|---|---|---|---|
| P-1 | CLI 冷启动（`--version`） | ≤8ms | ≤20ms | callgrind 指令数 + wall 预算 |
| P-2 | 万字全链路（md→方言→normalize→validate，无上传） | ≤60ms | ≤150ms | bench_convert |
| P-3 | 单图 2MB 解码 + 宽高探测 | ≤5ms | ≤15ms | mmap + 头流式解码 |
| P-4 | GUI 首帧可交互 | ≤400ms | ≤800ms | Tauri 启动打点 |
| P-5 | GUI 输入增量重渲染 | ≤8ms | ≤16ms | 60fps，仅脏 block（§3.2） |
| P-6 | 引擎常驻内存 | ≤50MB | ≤80MB | 内存预算 assert |
| P-7 | CLI 二进制体积（单平台） | ≤8MB | ≤15MB | CI size gate |

**B. 工程手段**
- 编译：`opt-level=3` `lto="fat"` `codegen-units=1` `panic="abort"` `strip=true`；feature 最小集（CLI 不链接 GUI 相关代码）；
- 解析/规范化：pulldown-cmark 流式 + 单遍 DOM visitor；规则位掩码快速预筛再精解析；**零 regex**（样式属性手写状态机解析）；
- 增量：IR block 内容哈希 + memoize，文件监听仅重算脏块（P-5）；
- 并发：rayon 数据并行（批量图片）、tokio（上传/HTTP IO）；剪贴板与 HTTP 零拷贝字节传递；
- 内存：bumpalo arena 承载 IR 与 HTML 字符串，避免逐节点堆分配；
- 网络：分片续传 + mmbiz 探测结果缓存（进程内缓存目录，按内容哈希）+ 限速退避。

**C. 门禁与文化**
- criterion + iai-callgrind 基准全表进 CI，**回归 >5% 阻断 PR**；
- SLO 同步维护在 `docs/performance.md`，README 公布分平台实测数字（附 CI 机器配置）——性能是全球开源的信任状；
- 无"感觉优化"：任何性能改动 PR 必须附 bench diff。

---

## 6. CLI 接口设计（Agent 的主入口）

```
wxwright convert <input.md|->      --theme <name|path>   --out <file.html|->   [--json]
wxwright validate <input.html|file.md> [--json] [--strict] [--official-check]
wxwright fix     <input.html>       --out <fixed.html>   # normalizer 报告模式
wxwright copy    <input.md>         --theme <name>       # 全链路：转换→mmbiz→校验→剪贴板 [--dry-run]
wxwright image   upload <paths...>  # 手动素材上传，输出 mmbiz 映射表
wxwright draft   create|update|list # 公众号草稿箱
wxwright publish <draft_id> --yes   # 群发（P2）
wxwright theme   list|new|validate  # 主题脚手架（模板开发用）
wxwright doctor                     # 环境体检：剪贴板/网络/凭据/node(官方CLI)/字体
wxwright mcp     serve              # 启动本地 MCP server（stdio）
wxwright agent-card [--md|--json]   # 「Agent 接手卡」：工具合同+规则红线+5 个示例命令，贴给任意 Agent 即刻接手
wxwright mcp     install --target <claude|cursor|vscode|opencode>  # 一键把 MCP 配置写入对应客户端
全局：-v --verbose  --json（机器可读）  --lang <en|zh-CN|…>（人类输出语言，默认按环境检测，§3.7）  退出码约定：0 成功 / 1 违规 / 2 运行异常（与官方 CLI 对齐）
```
约定：所有命令 `--json` 输出稳定 schema（版本化于 §7 manifest），非 TTY 时默认 JSON；stdin `-` 全支持。这些是 Agent 集成的接口合同，任何破坏性变更走 deprecation 流程。

**零配置主路径（上手即入门）**：无凭据即可用 `convert / validate / copy`（图片按 I-03 降级并给明确清单），配置凭据后解锁 mmbiz 与草稿——第一条命令就能到剪贴板。
**agent-card**：`--md` 输出面向人类粘贴的整段卡片（放 README「For AI Agents」一键复制块）；`--json` 输出机器可读合同（工具 schema + 规则红线摘要 + 退出码语义）。卡片内容与 §6/§7 实际实现做快照测试，防漂移。

## 7. MCP server 设计（第二 Agent 入口）
Tool 集（名称/入参对齐 CLI 语义，返回 `content[0].text` = JSON + `structuredContent`）：
- `wxwright_convert`：md→公众号 HTML；返回 html 字符串（>64KB 时 `html_truncated=true` 并提示改用 CLI `convert --out` 获取全量产物）
- `wxwright_validate`：HTML/MD 校验，返回违规数组
- `wxwright_copy`：写入用户本机剪贴板（**面向"人机接力"场景：Agent 备好文章，人只做粘贴**）
- `wxwright_upload_images` / `wxwright_draft_create` / `wxwright_draft_list` / `wxwright_themes_list`
- 资源（resources）：`wxwright://themes`、`wxwright://spec/rules`（把 §5.3 规则表作为可读资源喂给 Agent，让 Agent 自己知道边界——这是"喂给 Agent 的指令"的产品化）
- 提示（prompts）：`wxwright-publish-guide`（教未接入过的 Agent 一键发文的标准流程模板）
- 语言约定：工具名/入参/描述与错误机读部分恒为英文（供 Agent 消费）；`wxwright://spec/rules` 资源附中英双语说明（§3.7-E）

## 8. 主题/模板系统（生态位）
- 主题 = TOML：`[meta]`（名称/作者/许可）+ `[block.<role>]`（paragraph/h1…h6/code/table/image/blockquote/card-*）声明内联样式字典 + `[color_dark]` 建议色板（对比度预计算）；
- 编译期校验：主题经 `wxwright theme validate`（=validator 跑主题样例输出）才进仓库——**社区主题不可能引入违规样式，规范下沉到脚手架**；
- 内置组件（卡片/提示块）同样以"经过规范证明的 HTML 片段 + 参数槽"定义，允许 fork。

## 9. 验收标准（发布门禁）
1. **机器门禁**：golden 用例集（每功能 ≥2 例：正文/表格/代码/公式/多图/卡片/长文）全部 `wxwright validate --strict` 通过，且在 CI 中 `verify-article-structure-spec npm run check --json` 退出码 = 0；
2. **人肉四视图**：每主题每季度一次回归——公众号编辑器、PC 已发布、iOS/Android 已发布、Dark Mode 切换，逐项对照目检清单（仓库内 `docs/manual-test-matrix.md`，含截图存档 issue）；
3. **零失真定义用例**：Agent 端到端脚本（模拟：生成 md→MCP copy→人工粘贴→截图 diff）一次通过 ≥95%；
4. **三平台冒烟**：Win/macOS/Linux 的 CI 矩阵（cargo test + 剪贴板往返测试 + GUI 启动烟测）；
5. 规则表 §5.3 每条规则至少 1 个"违规样例 → 被检测/修复"的测试。
6. **多语言验收**：locale key-parity CI 通过（各语言包键集 ≡ en 源集，缺失回退英文）；en / zh-CN 双 locale 下 golden 报告快照测试通过。
7. **性能门禁**：§5.7 SLO 全表 CI 通过，基准回归 >5% 阻断 PR。
8. **设计治理扫描**：全库 emoji 扫描 = 0 命中（含 README/文档/主题/CHANGELOG）；GUI/CLI 图标 100% SVG 且引用 tokens 单一来源。
9. **即上手验收**：干净环境 ≤3 条命令走到「粘贴进公众号编辑器成功」（脚本 + 录屏留证）；`agent-card` 快照与 §6/§7 合同一致。

## 10. 里程碑
| 版本 | 范围 |
|---|---|
| v0.1 内部骨架 | core：parser+方言渲染+R-1/2/3 归一化+validator；CLI convert/validate；默认主题；i18n 消息框架（en 源语言 + key 契约，§3.7-B）；**设计 tokens + wxwright-icons SVG 起步（§3.8）；性能基准骨架 P-1/P-2/P-6/P-7（§5.7）** |
| v0.5 | 图片管线（data-w+上传+mmbiz，进程内限速重试+探测缓存）、copy、3 主题、MCP serve、三平台 CI + 官方 CLI 门禁；**agent-card + mcp install；出进程校验桥（--official-check，仅 CI）** |
| v0.9 | GUI（Tauri 双栏预览+Dark 预览，**Minimal 设计系统落地**）、fix 命令、代码/公式完整方案、manual-test-matrix、zh-CN 语言包；**增量渲染 P-5 达标、SLO 全表进 CI（§9-7）** |
| **v1.0** | 全 P0+P1；draft create；文档站双语（en/zh）；npm 可选桥（官方 CLI 集成）；GitHub 发布（§12）；**README「For AI Agents」+ 实测性能数字公开；emoji 扫描全绿（§9-8）** |
| **v1.5（已交付）** | 桌面客户端全量 GUI 增强（§15）：文章库/AI 助手（DeepSeek 式 harness）/Agent 面板/海报工坊/尺寸工坊/素材库/SVG 组件库（含 AI 生成与自定义图片）/ComfyUI+云端双绘图源/真机样机/写作宠物/i18n（en/zh） |
| v1.x | publish、主题市场（索引型，不开收费）、Windows ARM、性能（万字符长文 <2s） |
| v2 探索 | Android/iOS FFI、多平台扩展（小红书/知乎方言渲染=同一 normalizer 思路的复制品） |

## 11. 合规、安全与法律边界（开源前必须写清）
- **公众号接口**：本工具仅代用户操作其自有账号，需用户自备 AppID/Secret 并自行遵守《微信公众平台运营规范》与接口使用协议；README 提供资质/权限清单（草稿箱/素材接口对账号类型的要求）。
- **剪贴板/自动化粘贴**：不模拟登录态、不注入公众号页面脚本，仅"写入标准剪贴板制品 + 人/用户自己的自动化工具完成粘贴"，规避对编辑器本身的自动化干预风险。
- **第三方内容模板**：不搬运 135/秀米等商业模板；内置模板全部原创，社区主题贡献需附原创声明。
- **遥测**：零遥测、零云端依赖（本地优先）；崩溃上报 opt-in。
- **命名决议（v1.4 定稿）**：项目定名 `wxwright`（wx=微信 + wright=工匠，读 /ˈwɛksraɪt/，中文昵称"微信匠"）。全渠道查重（2026-09-24）：GitHub **仓库** 0 命中；crates.io / npm / PyPI 均无占用；`wxwright.com` 域名未注册；全网无同名产品。两项遗留处置：① GitHub **用户名** `wxwright` 被一 2016 年注册、零仓库的闲置账号占有——不影响组织/个人命名空间下建仓（`<you>/wxwright`），若需全局统一可向 GitHub 申请闲置用户名回收；② 旧草稿名 WeDraft 存在同品类先例（`pafa/WeDraft`）与游戏领域同名应用（Overwolf），定名后不再使用 WeDraft 作别名；③ 商标建议：发布前做一次"wx 前缀 + 腾讯商标族"的近似性专业复核（风险低，但开源项目命名属发布 blocker 项，查证据此归档）。

## 12. GitHub 开源规范与功能完整性（README 合同）
### 12.1 README 骨架（按此写，Agent 与人类双读者友好）
```
wxwright — 让任何 AI Agent 一键把文章发进微信公众号（Rust · CLI · MCP · 多端）
[badges: CI 三平台 / crates.io / license / MCP ready / 官方规范对齐 / SLO 性能预算 / No-emoji·SVG icons]
▸ 语言矩阵：English | 简体中文（README.md 英文主文档 + README.zh-CN.md 顶部互链；docs 站 /en/ /zh/ locale 切换；截图按语言版本）
▸ 30 秒上手（安装一行 → copy 一行 → 去公众号粘贴；GIF 演示）
▸ For AI Agents（一键接手）：复制 `wxwright agent-card --md` 整段卡片贴进任意 Agent 系统提示即接手；MCP 客户端一行接入（`wxwright mcp install --claude|cursor`）
▸ 设计系统与性能：大厂极简默认、全 SVG 图标、产品面零 emoji；公开各平台实测 SLO 数字（§5.7，附 CI 机器配置）
▸ 为什么不同：引擎化 + 官方规范硬门禁（对比 markdown-wechat 系/在线工具，表格化）
▸ 快速开始：CLI / MCP 接入（Claude Desktop、Cursor 配置片段） / GUI 下载
▸ 架构图（§3.2 同款） + 一条 curl/Agent 全链路示例（含 --json 输出真实样例）
▸ 零失真的 4 个条件（§3.5）——技术信任状
▸ 官方规范支持矩阵（§5.3 规则表人类可读版 + "哪些可自动修复"）
▸ 主题列表与自定义指南 ｜ 平台与安装矩阵 ｜ 安全与凭据 ｜ FAQ（图片为什么必须上传、为什么不支持拖拽排版等）
▸ 贡献（规范即测试、新增主题流程、i18n） ｜ 许可证 ｜ 免责声明
```
### 12.2 仓库工程规范
- 许可：MIT OR Apache-2.0（Rust 生态惯例，双授）；
- CI：`.github/workflows`——三平台 cargo test/clippy/fmt + golden 校验 + **官方 puppeteer CLI 门禁**（node20 setup）+ locale key-parity 检查（§9-6）；GUI 构建 nightly；
- 模板：Issue 含"违规样例 HTML 复现单"专版；PR 含"新增/修改规则的规范出处链接"硬要求；
- CHANGELOG（Keep a Changelog）、语义化版本、`cargo release` 流程、crates.io 发布（core 可单独复用是给 Agent 开发者生态的钩子）；
- 文档站：mdbook 或 VitePress（主题开发指南、Agent 集成指南、规范解读）；
- releases 提供三平台预编译二进制 + 签名（macOS notarize）；
- `docs/agent-integration.md`：专供 Agent 的机器友好集成手册（工具合同、退出码、JSON schema），与本文 §6/§7 同步。

### 12.3 分发形态矩阵（免安装为默认，安装包为可选）
| 平台 | 主形态（默认） | 安装通道（可选包装） | 备注 |
|---|---|---|---|
| Windows | **单文件 portable `wxwright.exe`**（x86_64；aarch64 随后） | `winget install wxwright`（开源 CLI 事实标准）；install.ps1 一行脚本放入 PATH；MSIX/Inno 仅 GUI 版按需 | CLI 核心不提供传统 installer，保证零解压/零注册/退出无痕（§1.5） |
| macOS | Universal 二进制（x86_64+arm64） | `brew install wxwright`；curl 一行脚本 | **必须 Developer ID 签名 + notarize**，否则 Gatekeeper 拦截（苹果分发要求，与形态无关） |
| Linux | 静态链接 ELF（musl 目标，零 glibc 依赖） | deb/rpm/AUR/Nixpkgs 由社区打包 | releases 附 sha256 校验和 |

- 所有 releases 同时提供：裸二进制 + 校验和 + 签名（minisign/sigstore）——**下载即用是产品人格的一部分**，与"装完即用、退出无痕"的极简架构承诺一致；
- 更新检查不做静默常驻：仅 `wxwright doctor` / 显式 `--check-update` 触发一次性请求（零常驻原则 §3.2）。

## 13. 风险登记册
| 风险 | 等级 | 缓解 |
|---|---|---|
| 编辑器粘贴行为随公众号前端升级变化 | 高 | 四视图人肉回归季度化；validator 以官方仓库为真值持续对齐；golden diff CI |
| 官方规范附件截断（§4.4 SVG 细则） | 高 | **实施前从 `verify_article_structure.md` 拉权威全集，§5.3 映射表补全后才算 v0.1 完成** |
| 素材接口权限/账号类型门槛 | 中 | 双路径设计（剪贴板无需凭据）；README 权限矩阵 |
| 固定 px→百分比自动换算的视觉偏差 | 中 | 换算表保守（仅宽度类属性），fix 模式逐条可关 |
| Dark Mode 规则与创作自由冲突 | 低 | 警告不阻断，提供 --fix-dark |
| 移动端剪贴板 OS 限制 | 低 | v1 桌面专属 copy，移动端定位=预览/导出（§3.6 已声明） |
| 翻译漂移 / 各 locale 键不齐 | 中 | en 为源语言 + key-parity CI 阻断 + 缺键回退英文；机读面永不本地化（§3.7-B） |

## 14. 开放问题（需产品/技术决策）
1. ~~项目命名定稿~~ **已定稿 `wxwright`**（2026-09-24，查重与处置见 §11；GitHub 同名闲置用户名回收为可选项）；
2. GUI 是否 v1 与 CLI/MCP 同步发布（当前里程碑 v0.9），若人力紧张建议 CLI+MCP 先行、GUI 后置——与"Agent 基础设施"定位一致；
3. 公式 SVG 模式的 Dark 不处理是否可接受（默认走图片化）；
4. 是否需要"内网/代理上传"模式（企业公众号素材接口出口限制场景）。
5. zh-TW / ja 是否随 v1.0 首发（视发布前社区招募情况）；翻译协作平台选 Weblate 还是 Crowdin（或纯 PR 贡献）。
6. **可选 daemon 组件的引入时机**（并发上传共享 / 多端凭据缓存场景）——仅当有真实需求证据后作为附加可选组件评估；默认永远单进程（§3.2），引入即触发 §5.7 预算重估。

---
## 15. 已交付能力对照（v1.0 → v1.5 GUI 增量）

以下能力均已实现并通过浏览器视觉/交互验收（对照 §4 功能需求，新增部分标注为 GUI 增量）：

| 能力 | 说明 |
|---|---|
| 桌面客户端（Tauri 2） | 三栏布局：文章库 / 编辑器 / 真机样机预览；拖拽载入 .md |
| 文章库 | 本地 Markdown 存储（frontmatter 元数据）；新建/保存/重命名/复制/导入/搜索/删除；Ctrl+S；切换自动保存 |
| AI 助手 | DeepSeek 式 harness：SSE 流式、思考中指示、增量渲染、滚动跟随+回底、Markdown 渲染、消息元信息（模型·时长·tokens）、真停止、错误气泡；快捷指令（润色/续写/起标题/提纲/头图文案）；回复一键插入/替换/复制；附件上传（.md/.txt 作为上下文）；模型快选（输入框内切换 Provider） |
| AI Provider | OpenAI 兼容协议（OpenAI/DeepSeek/通义/Kimi/GLM/Agnes 等 + 本地 Ollama/LM Studio）；Key 入系统钥匙串（keyring 失败回退用户目录文件）；测试连接；用量统计 |
| Agent 面板 | MCP 一键写入 Claude Desktop/Cursor/VS Code/OpenCode；Agent 接手卡一键复制；CLI 速查 |
| 主题系统 | 三内置主题 + **AI 生成主题**（描述→TOML→官方规范校验→失败自动重试→存用户主题目录，CLI 同样可用） |
| 海报工坊 | HTML→PNG 本地光栅化（foreignObject）；公众号全尺寸预设（头图 1080×460 等 6 种 + 自定义）；AI 生成海报 HTML；导出并插入文章 |
| 尺寸工坊 | 任意素材裁切/补白适配微信全部标准尺寸（含贴图 900×383/383²），1x/2x 导出 |
| 素材库 | 统一管理海报/AI 图/截图/拖入图片；后端缩略图；插入/尺寸工坊/删除；打开文件夹 |
| AI 绘图双源 | **ComfyUI（本地）**：自动探测、一键启动（launch_path）、文生图/图生图；**云端图像 API**：OpenAI images 协议（如 Agnes image 模型）；产物统一进素材库 |
| SVG 组件库 | 6 个参数化互动组件（含自定义图片上传组件）；**AI 生成组件**（专家提示词 + 合规校验自动重试）+ 自定义组件持久化；插入后引擎复核 |
| 真机样机 | iPhone 15 Pro（灵动岛/侧键/Home 条）/ Pixel 8（打孔）真实硬件绘制；样机下方双按钮切换；深浅色在样机上切换；滚动条隐藏 |
| 合规徽标 | 状态行彩色徽标（阻断/提示计数）+ 明细面板（不遮挡 AI 助手） |
| 写作宠物「墨仔」 | 独立组件（pet.js 自包含，形态内嵌 data URI 无路径依赖）；AI 生成的 5 形态（坐姿/敲键盘/瞌睡/跳跃/派对）；空闲小剧场轮播；打字/保存/复制状态反馈；点击摸摸（红心+语录）；双击换装；logo 三连彩蛋；状态栏「码聋」弹出公众号二维码 |
| i18n | 界面 en/zh-CN 切换（顶栏按钮）；缺 key 回退 key 名（不显示 undefined） |
| 图表引擎 | ```chart 围栏 JSON（bar/line/pie，标题/标签/数值/单位）→ 内联自包含 SVG 图表（677×430，6 色板），随正文一并通过官方规范校验；AI 系统提示词已接入 schema，AI 可直接输出数据图表 |
| 公众号 API 绑定 | 设置弹窗重构为左侧导航四分区（AI Providers / 公众号 API / ComfyUI / 关于）；AppID/AppSecret 入系统钥匙串（wxwright-mp），状态脱敏显示，支持解绑 |
| 交互润色 | 校验按钮移至保存旁（胶囊样式）；SVG 组件库颜色参数内置取色器；AI 插入/替换带编辑器落点高亮 + 预览末块脉冲；墨仔形态切换交叉淡入；应用图标超采样重生成（1024px 渲染→Lanczos3→8 帧 ICO）；官方品牌标扩至 17 家（新增 xAI、智谱） |
| 统一 AI 生成运行时 | 全部生成面板（AI 主题/SVG 组件/海报 HTML/云端生图/ComfyUI）收敛为 job 运行时：单一 ai-job 事件通道流式输出（含 DeepSeek 式思考指示）、真实取消（SSE 读循环断连 + ComfyUI /interrupt）、共享运行控制台（状态/耗时/停止/流式日志）、多 job 并发；主题/SVG/海报共用「生成→提取→校验→修复」规格表，海报新增自包含门禁（外链/脚本/字体在光栅化前拦截并回喂修复） |
| 推理模型能力注册 | 按模型 id 启发式识别思考型模型（o1/o3/r1/reasoner/glm-z/qwq 等）自动放大起始预算；空正文截断时预算阶梯加倍（封顶 16384）；`<think>` 块剥离 |
| 平台出口适配与规则表 | 复制主按钮按平台产出对应制品：微信=方言富文本、小红书=纯文本文案（IR 线性化：列表转圆点/图片转占位/链接只留文字）、知乎=原样 Markdown；合规徽标按平台规则表切换（小红书 XHS-1..5：标题≤20/正文≤1000 阻断/话题标签/图组提示/图片≤9）；platform 字段随文章 frontmatter 持久化，打开文章自动恢复，切换即标记未保存 |
| 平台注册表（§16 落地） | wxwright-core::platform 六平台描述符（微信公众号/小红书/知乎/Meta/X/LinkedIn）：能力位（富文本/图片笔记/API 发布）+ 预设尺寸 + 能力注记；CLI `wxwright platforms` 列出；GUI 顶栏平台切换器联动海报/尺寸工坊预设与 AI 文案风格；小红书「导出图组」一键按平台预设批量光栅化入素材库 |
| 发布自动化 | Tag 驱动 GitHub Actions：Windows NSIS+便携、macOS dmg+CLI 双架构、Linux deb/AppImage+musl CLI、SHA256SUMS（docs/release-automation.md） |

## 16. 海外社交媒体扩展方向（v2 评估）

核心思路（PRD §3.1 不变）：**同一颗引擎，多套「方言渲染器 + 合规规则表」**。wxwright-core 的 parser/IR/模板层与平台无关，新增平台 = 新增 render 目标 + 该平台规则表 + 剪贴板/API 出口适配：

| 平台 | 差异要点 | 复用度 | 优先级 |
|---|---|---|---|
| 小红书 | 无富文本 API，主路径为「图片笔记」→ 海报工坊/尺寸工坊直接产出 3:4 封面与正文图组；文字走官方发布器 | 高（图片管线全复用） | 高 |
| 知乎 | 富文本/Markdown 原生友好，规则宽松；需要文章 API 适配 | 高（渲染器薄壳） | 中 |
| Meta（Facebook/Instagram） | Graph API 文本+图片；Instagram 以图片为主（尺寸工坊出 1:1/4:5/9:16）；X 走 API v2 发文（280 字限制需摘要策略） | 中（出口适配为主） | 中 |
| LinkedIn | 文章 API 成熟，富文本有限 | 中 | 低 |

前置条件：各平台 API 资质与合规审查（参照 §11 的边界声明逐平台补齐）；GUI 的平台切换器（当前为微信专用）预留于 §3.2 架构中。

*附：本文档引用的官方规范要点来自用户提供的附件全文（前言、§1 CSS 属性、§2 文章结构、§3 字体、§4 Dark Mode 及 verify CLI 说明）；实施前须以 `github.com/wechatjs/verify-article-structure-spec` 仓库内 `verify_article_structure.md` 为权威全集复核。*
