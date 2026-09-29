# AGENTS.md — wxwright AI Agent 接管指南 / Agent Takeover Guide

> 本文档面向任何接管本项目的 AI Agent（Claude Code / Codex / ZCode / OpenCode / 人类工程师）。
> 目标：读完本文档即可安全地继续开发，不重复踩坑。改完任何东西后，若发现新坑，**必须更新本文档第 7 节**。

---

## 1. 项目是什么 / What this is

wxwright 是**面向社交媒体创作的桌面客户端 + Rust 引擎**：一条 Markdown，多渠道分发。
核心理念（PRD §3.1 / §16）：**同一颗引擎，每个平台 = 一套「预览形态 + 合规规则表 + 导出物」**。

- 唯一信源：文章库里的 Markdown（frontmatter 含 title/theme/platform）。
- 渠道三件套（切换平台时整体切换）：
  - 预览形态：微信=方言长文滚动 / 小红书=笔记壳（图组+文案）/ 知乎=中性排版文章。
  - 规则表：微信 R-1~R-4（官方规范）；小红书 XHS-1..5；知乎宽松。
  - 导出物：微信=方言富文本复制/推草稿；小红书=纯文本文案+图组；知乎=Markdown。
- 当前平台注册表：**7 个**——微信公众号 / 小红书 / 知乎 / Facebook(Meta) / Instagram / X / LinkedIn
  （`wxwright-core/src/platform.rs`；`PlatformSpec` 含 `export_kind` 行为字段，描述符是数据不是行为）。
- 作者品牌：**AI瑶 · 公众号「码聋」· github.com/YaoIsAI**（不得改动/移除）。

## 2. 仓库结构 / Layout

```
crates/wxwright-core/    引擎：parser(IR) / render(方言) / normalizer / validator(R-规则) /
                         theme(TOML) / platform(平台注册表+出口适配) / img / clipboard / htmlutil
crates/wxwright-cli/     CLI：convert / validate / fix / copy / draft / publish / platforms / doctor / agent-card / mcp
crates/wxwright-mcp/     MCP stdio server（9 工具，手写 JSON-RPC，per-request catch_unwind）
crates/wxwright-mp/      公众号 API（凭据 keyring / 草稿 / freepublish / mmbiz 上传）
gui/src-tauri/           Tauri 2 桌面客户端后端（Rust；不是 crates/ 下的成员）
gui/src-tauri/src/       ai.rs(Provider/SSE/complete) · jobs.rs(统一 AI 生成运行时) · social.rs(海外平台 BYO 一键登录/凭据) · comfy.rs(ComfyUI) ·
                         articles.rs(文章库 frontmatter) · extract.rs(PDF/DOCX/HTML 文本提取) · commands.rs · lib.rs
gui/ui/                  index.html · app.js(主逻辑,~4100行) · ai-jobs.js(生成任务桥) · pet.js(宠物墨仔) ·
                         brand-logos.js(17厂商+5平台官方矢量,内嵌 path/dataURI) · styles.css · qrcode.jpg
tools/icongen/           图标生成（1024px 超采样→Lanczos3→8帧 ICO）
dist/                    发布目录（两 exe；gitignore；**发版必须手动同步**）
PRD.md / CHANGELOG.md / AGENTS.md / docs/
```

## 3. 常用命令 / Commands

```bash
cargo test --workspace                       # 全量测试（当前 163 通过 + 6 条 live ignored，必须全绿才能交付）
cargo test -p wxwright-gui live_theme_generation_smoke -- --ignored --nocapture
                                             # 真实 API 冒烟：AI 生成主题端到端（花 token，需已配 Provider）
cargo build --release                        # 发布构建（~7 分钟）
cargo run -p wxwright-cli -- convert a.md --out a.html
cargo run -p wxwright-cli -- validate a.md --strict   # 退出码 0/1/2
cargo run -p wxwright-cli -- platforms       # 平台注册表（非 TTY 自动 JSON）
cargo run -p wxwright-cli -- convert a.md --platform xhs   # 非微信平台的制品；会一并报该平台文案规则，
                                             # 命中 block 级违规时退出码 1
cargo run -p wxwright-cli -- doctor --strict # CI/SCP 前置门禁（有问题退出码 1）
cargo run --release -p icongen               # 重生成图标
python -m http.server 8742 -d gui/ui         # 浏览器 demo 模式（无后端）验收 UI
```

**交付流程**：`CARGO_TARGET_DIR=E:/wxwright-build cargo build --release` → `taskkill //IM wxwright-gui.exe //F; sleep 2` →
`cp E:/wxwright-build/release/*.exe dist/`（**没有 /target 段**；C 盘 target 里是 0.10.0 时代旧 exe，见坑 33）→ 重启 `dist/wxwright-gui.exe` → git commit。
**用户启动的就是 `dist\wxwright-gui.exe`**，忘记同步 dist = 用户看不到任何修复。桌面端验收前端时，用
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9333"` 启动 + CDP `Runtime.evaluate` 查 `script[src*=app.js]` 的 ?v= 版本，确认资产真的进 exe。

## 4. 架构铁律 / Hard invariants（违反即事故）

1. **Tauri 命令阻塞主线程 = 卡死**。一切阻塞 IO/网络必须 `async fn` + `spawn_blocking`。
2. **命令在 commands.rs 定义后必须注册进 lib.rs 的 generate_handler**，否则是死代码
   （线索：模块内函数报 never-used 警告）。**已由 `tests/gui_governance_test.rs` 自动守卫**：
   它 diff「带 `#[tauri::command]` 的函数」与「`generate_handler!` 里的清单」，双向断言。
   临时把某条注册摘掉会让测试红（已做反证检验）。
3. **no_emoji 测试**（`crates/wxwright-core/tests/no_emoji_test.rs`，PRD 3.8-B）：产品文件
   （代码/HTML/CSS/JS/AGENTS.md 等所有入库文本）禁止一切 emoji，连勾选框符号
   U+2611/U+2610 都拦（本文件第 7 节就是被它抓出来的）。文案用文字或内联 SVG。
4. **`t()` 缺 key 必须返回 key 名**，绝不显示 undefined；新 UI 字符串必须同时补 zh+en 两个字典
   （gui/ui/app.js 的 I18N），静态 HTML 用 `data-i18n` / `data-i18n-placeholder` /
   `data-i18n-title` / `data-desc-i18n`（applyI18n 统一处理）。
   **已由 `tests/gui_governance_test.rs` 自动守卫**：比对两个字典的 key 集合是否一致，
   并检查 index.html 里每个 `data-i18n*` 引用的 key 在两个字典里都存在。
   注意 I18N 的值可能是箭头函数 + 模板字符串（`key: (id) => \`...\``），守卫测试用
   「先剥离字符串字面量再扫 `ident:`」的方式解析，不会把值里的冒号误当 key。
5. **前端子资源有 cache-bust**：index.html 引用 `?v=N`。**每次改前端文件必须递增该文件的 v**
   （styles.css / app.js / ai-jobs.js / pet.js / brand-logos.js 各自独立）。
6. **密钥隔离**：`agnes ai.txt`（明文 key）与 `dist/`、`settings.json` 被 .gitignore 排除；
   API Key 只进系统钥匙串（keyring，失败回退 %APPDATA%，仓库外）。任何密钥不得入库/入文档/入日志。
7. **官方 logo 库**（brand-logos.js）：Simple Icons CC0 抓取（404 响应体会被写成假 svg，
   必须 grep `<path` 验证）；Simple Icons 没有的（xAI/Zhipu/LinkedIn）取 Wikimedia 官方素材
   并在 CHANGELOG 注明来源。不要手绘品牌标。
8. **SVG 组件插入必须作为文档末尾独立块**（光标插入会嵌进列表/代码块导致渲染失效）。
9. **MCP 工具面**（9 工具）由 `tool_surface_matches_agent_card` 漂移测试锁定：
   改工具清单必须同步 agent card + README，否则 CI 红。
10. **修改 logo/图标**需三处同步：master.svg、index.html symbol、icongen 重跑。
11. **主题角色以 `wxwright-core/src/roles.rs` 为唯一事实源**。渲染器、AI 主题提示词
    （`ai::theme_schema()`）与 `validate_generated_theme` 三方都从 `ROLES` 取。
    **新增/改名的 role 必须三处同时成立**，`tests/theme_roles_test.rs` 会给每个 key
    写入一条独特声明并断言它出现在渲染结果里——「提示词教一套、渲染器认另一套」
    会直接测试失败（这条铁律来自 card_* 角色被静默丢弃的事故）。
12. **剪贴板/草稿的图片门禁只有一处实现**：`ImageOutcome::paste_hostile()`。
    共有**五个写出口**必须全部用它：CLI copy（`main.rs`）、MCP copy、MCP draft_create、
    GUI copy、GUI 推草稿。`tests/image_gate_test.rs` 会扫描源码，
    任何手写的 `inlined && !mmbiz` / `starts_with("data:")` 判断都会让测试红。

13. **```html 围栏是 AI 的富样式通道，白名单是唯一的安全模型**：
    `htmlutil::html_block_allowed` / `markup_allowed`（allowlist，不是 blocklist——黑名单只防住作者想得到的那几种）。
    放行的标签表就是方言词汇表；**`img` 和 `a` 被有意排除**（前者绕过素材门禁、后者 MP 不保留链接）。
    通过白名单的 HTML 原样进渲染输出（**三种入口同一判定**：```html 围栏、裸块级 HTML、行内片段——模型实测永远写裸 HTML，提示词教不会，引擎必须兜住），再由管线统一 normalize；不通过的转义/降级代码块。
    **SvgEmbed 准入同用此扫描器 + SVG_EMBED_TAGS 词表**（旧版是 10 条黑名单，`onmouseover` 漏过，勿回退）。
    AI 主题/组件提示词若要教模型用 HTML，教的就是这张表。
## 5. 统一 AI 生成运行时 / jobs.rs（新功能一律走这里）

- 任何 AI 生成 = 一个 job：`ai_job_start(kind, params)` → 事件 `ai-job {id,kind,ev,data}`
  （ev = status|delta|think|done|error）→ `ai_job_stop(id)` 协作取消（SSE 读循环断连 / ComfyUI /interrupt）。
- kind：`theme` | `svg` | `poster` | `image` | `comfy`。chat 任务共用规格表
  （system prompt → extract → validate → 回喂修复，预算阶梯封顶 16384）。
- **前端交互规范（用户明确要求）**：触发按钮自身变「停止」（ai-jobs.js 的按钮态模式，
  capture 阶段拦截第二次点击）。**禁止**新增控制台框/思考框/独立停止按钮。
- 推理模型（o1/o3/r1/reasoner/glm-z/qwq…）自动放大起始预算；空正文+length 截断自动加倍重试。

## 6. 验证流程 / Verification（用户对"声称修了但没修"零容忍）

1. 代码级：`cargo test --workspace` 全绿 + `node --check` 每个 JS。
2. 浏览器级：`python -m http.server 8742 -d gui/ui` + 截图验收布局/交互
   （注意：demo 模式无 Tauri 后端，AI/附件/绑定等会提示"演示模式不可用"，属预期）。
3. 桌面级：release 构建 → 同步 dist → 重启 → 用户实际点击路径走查。
4. **每个修复必须给用户证据**（截图/DOM 查询结果/测试输出），不接受"应该好了"。

## 7. 踩坑清单 / Pitfalls（历史事故，勿重蹈）

| # | 坑 | 后果 | 正确做法 |
|---|---|---|---|
| 1 | **bash heredoc 写 python 补丁**：`\\n` 折叠、引号截断 | 补丁静默失败/半途 abort | 多行补丁一律 Write 工具写 .py 再执行，每个 replace assert |
| 2 | **补丁把新函数插文件顶部、旧定义残留** | JS 后定义覆盖前者，新功能是死代码（addAttachment 事故：多格式附件半年不生效） | 补丁必须先定位并删除/替换旧定义；交付前 `grep "^function X" \| sort \| uniq -d` 扫重复 |
| 3 | **init 重写裁掉尾部函数** | 死引用（renderComfyStatus 等四函数事故、logo 彩蛋丢失） | 重写大段代码后用 declared vs referenced 差集扫描 |
| 4 | **命令漏注册 lib.rs** | 整条链路死代码（extract_document_text 事故） | 见铁律 2 |
| 5 | **忘记同步 dist/** | 用户看不到修复 | 见交付流程 |
| 6 | **前端缓存** | 改动不生效 | cache-bust 递增（铁律 5） |
| 7 | **伪造成功** | 用户信任崩塌 | 未验证 = 未完成；证据先行 |
| 8 | **浏览器 demo ≠ 桌面端** | demo 里 AI 全部"不可用"被当成 bug | 向用户说明边界；桌面功能走桌面验收 |
| 9 | **Tauri 同步命令** | 全 UI 卡死 | 见铁律 1 |
| 10 | **Simple Icons 404 假 svg** | logo 缺失 | grep `<path` 验证；缺失走 Wikimedia |
| 11 | **CSS 自造变量名** | 样式静默失效 | 用真实变量：--accent/--accent-soft/--bg-subtle/--text-secondary/--border/--err/--ok |
| 12 | **[hidden] 被 display:flex 覆盖** | 面板关不掉 | 全局 `[hidden]{display:none!important}` 必须保留 |
| 13 | **iframe sandbox 空** | 父页 evaluate 拿不到 contentDocument（安全特性） | 验证预览内容用 CLI convert --json 或 srcdoc 前的模型数据 |
| 14 | **demo 流式被后台 tab 节流** | 浏览器验收卡顿 | demoStream 已 chunk 化（24字/90ms），finalizeAiMessage 有 isConnected 守卫 |
| 15 | **raw 字符串 `"#` 提前终止** | Rust 编译错 | 用 `r##"..."##` |
| 16 | **API Key 入库** | 事故级 | 见铁律 6；每次 commit 前 `git check-ignore` 复查 |
| 17 | **用户会亲手改代码/文档** | 接管时假设过期 | 接管先 grep 版本号/关键字核对落盘，别信上次记忆 |
| 18 | **宠物形态切换** | 用户否决了交叉淡入（生硬） | 保持直接切换 `im.src = ...`，不要自作主张加过渡 |
| 19 | **AI 交互新增控件框** | 用户否决（要求极简） | 一律按钮态复用（生成中→停止），见第 5 节 |
| 20 | **native `<select>` 无法放 logo** | 平台下拉需求不达标 | 自定义 trigger+menu（参照 #platform-trigger / #ai-model-menu 模式） |
| 21 | **手机样机自带假状态栏** | 壳内再画 9:41 会双重叠加 | `.device-status` 透明覆盖 iframe 顶部（iOS 54px）；壳只留 34px 空白带（shellTop） |
| 22 | **demo 分支提前 return** | 渠道壳改动在 8742 浏览器里完全不可见，误判"没实现" | 非 invoke 分支同样接渠道壳（demoCaptionFromMd / demoPlainHtmlFromMd） |
| 23 | **新增平台只改 core** | demo 回退/菜单 logo/壳调度缺一处就半联动 | 加平台五处同步：core PlatformSpec → demo 回退 PLATFORMS → PLATFORM_LOGOS → SHELL_BUILDERS → 预设 |
| 24 | **Windows 下 `cargo test` 跑不了 mock_app 测试** | 测试二进制启动即 `STATUS_ENTRYPOINT_NOT_FOUND`（tauri#11028，DLL 入口点）；PATH 注入 WebView2Loader.dll 也无效 | 命令层测试走不需要 AppHandle 的纯函数/同步入口（如 `generate_theme`），并加 `#[ignore]` live 冒烟真实验证；jobs.rs 的 emit/chat_stream/run_chat_task 已改成 `R: tauri::Runtime` 泛型备用 |
| 25 | **HTML 重复 id 二次发生（comfy-launch-path / svgkit-desc）** | getElementById 只认第一个，后出现的输入框/span 成静默死控件 | 加弹窗输入框先 `grep -o 'id="[^"]*"' index.html \| sort \| uniq -d` 扫重；两个弹窗需要同字段时一个改名 + 双向同步 |
| 26 | **demo 存根 UI 没绑事件** | 素材存根卡的插入/工坊/删除按钮点击静默无反应，验收时被当 bug 报 | demo 回退的每个按钮要么绑真实 demo 行为、要么统一回 demo_mode toast，不留死按钮 |
| 27 | **CI 工具链钉在 1.96（dtolnay/rust-toolchain@1.96）而本地已可 rustup update** | 新 clippy（如 1.98）会引入新 lint，CI 与本地版本漂移导致「本地绿 CI 红」 | 升级步骤：本地 rustup update 后跑 `cargo clippy --workspace --all-targets -- -D warnings` 清零，再把两个 workflow 的 @1.96 升到新版本。1.98 已知待修：clippy::question_mark（theme 百分比解析 else-return）、clippy::unneeded_wildcard_pattern |
| 28 | **C 盘被 target 吃满（os error 112）两次复发** | cargo build/test 中途磁盘写失败，构建报 icongen/rustc exit 101 等莫名错误 | 报错先 `df -h /c` 查盘；`rm -rf target/debug/incremental`（3-11G）或整个 target/debug 速救；大型构建前预留 >10G |
| 29 | **`cargo run ... > file` 且构建失败时把目标文件写空** | 生成物（如 `gui/ui/demo-agent-card.md`）被截断成 0 字节，且 stderr 被 `2>/dev/null` 吞掉，看上去像「命令成功但没输出」 | 先输出到临时文件并判空再覆盖：`cargo run ... > /tmp/x 2>/tmp/x.err; [ -s /tmp/x ] && cp /tmp/x dest || cat /tmp/x.err` |
| 30 | **C 盘满到 `rm -rf` 之后可用空间反而更少** | 其他进程（系统更新/索引/备份）在同时吃盘，跟 C 盘抢空间没有胜算 | 直接换盘：`CARGO_TARGET_DIR=E:/wxwright-build cargo build --release`（E 盘 134G 可用），零风险且不影响 dist 同步 |
| 31 | **主题 role 声明了但渲染器不读** | AI 生成主题「合规却无效」，用户感知为「AI 排版没用」（card_* 事故：`let _ = row;` 式的 `let _ = role;`） | 见铁律 11；`roles.rs` + `theme_roles_test.rs` 双重锁定 |
| 32 | **卡片容器在标题后闭合，正文被 append 成兄弟节点**（render_card 事故） | 所有 callout（[!WARNING]/[!IMPORTANT]/[!TIP]/[!NOTE]）底色只盖住标签行，正文落白底，卡片视觉=细条；用户感知为「卡片效果没有」 | 正文块必须渲染在容器 section 内部；验证卡片要看「正文是否在色底上」，不能只 grep 背景色出现与否 |
| 33 | **dist 同步从 `C:/…/target/release` 拷贝**（CARGO_TARGET_DIR=E: 后 C 盘 target 是 0.10.0 时代旧 exe） | 新构建其实成功，但拷进 dist 的是旧 exe：Rust 修复丢失 + 前端嵌 v24——busy spinner（v26）、侧栏轮询（v27）连续两次「已交付」实际全没生效；测试还误判成「前端代码没跑」 | 一律 `cp E:/wxwright-build/release/*.exe dist/`（CARGO_TARGET_DIR 本身就是 target，**产物在 release/ 下，没有 /target 段**）；桌面端验收先 CDP 确认 `app.js?v=N` 是新版本 |
| 34 | **原子代际判定写成「fetch_add 的返回值 vs load 的新值」**（AI 停止按钮事故：`chat()` 用旧值作 my_gen，`stop()` 存新值，判定式恒不成立；停止从未生效，且残留 STOP_GEN 让下一次对话首行即中断） | 用户点停止无反应，或「刚发就停」；有提交标题声称修了但没碰判定式 | 代际算术抽成 `next_generation(&AtomicU64)` / `mark_stop_for_current()` 纯函数，用局部原子做**双向**单测（停止必须命中当前、空闲停止不得污染下一次）；交付前把 bug 放回去确认测试会红 |
| 35 | **契约测试只做前向断言**（`theme_roles_test` 只验「声明了的都被消费」，抓不到「渲染器支持但表里没声明」→ h6 漏网，`validate_generated_theme` 会拒绝一个合法 key） | 主题校验器拒绝一个实际生效的 role；同类问题在「动态拼 role」处（`format!("h{}", level)`）最容易发生 | 契约必须**双向**：前向用探针断言每个 key 生效；反向用端到端驱动枚举动态角色族（h1..=6）并断言「既生效又已声明」；再加算术自检测试锁住角色/key 数量 |
| 36 | **列表上限截断不区分严重度**（`MAX_VIOLATIONS=500` 一视同仁丢弃 → 501 条 warn + 1 条 `<script>` 被判 compliant，退出码 0） | 「有阻断违规」被静默变成「没问题」，零失真承诺失效；MCP 的 validate 同样中招 | 阻断级给独立上限（`MAX_BLOCK_VIOLATIONS`），warn 可以被截断，**block 永不被隐藏**；回归测试构造「超额 warn + 1 个 block」断言 block 仍在 |
| 37 | **同一判定在多条路径上各写一遍**（图片门禁 `inlined && !mmbiz` 曾在 CLI/GUI/MCP 各写一份，MCP 那份还漏 http 与读取失败；`wxwright_draft_create` 干脆一道门禁都没有） | 同一篇文章，四条路径给出四个不同答案 | 判定只留一处实现（`ImageOutcome::paste_hostile()`），全部写出口调用它；再加**源码扫描测试**（`tests/image_gate_test.rs`）让手写过滤无法复现 |
| 38 | **`ureq` 不读环境变量也不读 Windows 系统代理**（本机 Clash 在 127.0.0.1:7897，`curl` 走 `HTTPS_PROXY` 能通，应用内直连 Cloudflare 前置的境外端点全部 `os error 10060`） | 桌面端 AI 助手 / 云端生图 / X·LinkedIn 换 token **全部不可用**，但日志只显示「连接超时」，极易误判成「服务商挂了」或「密钥错了」 | 境外出口一律走 `gui/src-tauri/src/net.rs` 的 `with_env_proxy`（读 `HTTPS_PROXY`/`ALL_PROXY`/`HTTP_PROXY` + `NO_PROXY=*` 退出）。**本地（ComfyUI 127.0.0.1）与境内（微信 API）保持直连**——别开 ureq 的 `proxy-from-env` feature，它没有 NO_PROXY 支持且 `ALL_PROXY` 优先于 `HTTPS_PROXY`，会把回环请求也塞进代理 |
| 39 | **`cargo test ... \| grep "test result"` 会把编译失败伪装成「没有测试」**（本次实测：E 盘增量目录被瞬时占用 → `os error 5 拒绝访问` → 编译中止 → grep 无输出 → 汇总统计显示为空，差点被当成通过） | 静默漏测；比报错更危险 | 跑测试时**先看退出码**，或用 `set -o pipefail`；拿汇总数时断言它非空（`[ -n "$TOTAL" ] \|\| exit 1`）。Windows 上若出现 `os error 5` 占用增量目录，加 `CARGO_INCREMENTAL=0` 重跑即可（多为 Defender 扫描新写的 .rmeta） |

## 8. 当前能力快照 / Feature map（2026-09-26，v0.9.0+）

- 文章库（frontmatter 含 platform）/ 主题（3 内置 + AI 生成）/ 海报工坊 / 尺寸工坊 / 素材库 /
  SVG 组件库（6 组件+AI 生成+传图+取色器）/ AI 绘图双源（ComfyUI+云端）/ 真机样机（iPhone15Pro/Pixel8+深浅色）/
  宠物墨仔 / 合规徽标 / 渠道预览像素级分平台壳（wechat 方言 / xhs 笔记详情 / zhihu 文章页 /
  facebook 卡片 / instagram 帖子 / X 帖子 / linkedin 卡片，各按真实字号比例配色实现）/
  i18n / Agent 面板 / 公众号 API 绑定（GUI 推草稿按钮直通草稿箱）/ chart 图表引擎 /
  **主题变体**（role.variant 点分键：quote.hero 短金句 / paragraph.lead 首段导语，
  渲染期内容分类+双层合并，`roles::RoleDef.variants` 是唯一事实源，AI 主题提示词
  自动教变体）/ **AI 配图**（jobs kind `illustrate`：文本模型规划锚点插图位 → 云端
  生图 → 按平台画像 fit → 写回文章，工具栏火花按钮）/ **图像跨平台裁剪**
  （`img::fit_to_profile` + `PlatformSpec::image_profile()` + CLI `image fit`）。
- 测试 180（173 常规 + 7 条 live 冒烟 ignored）；版本 0.10.0（workspace+tauri.conf）。
- 多渠道出口：`convert --platform <id>` 与 MCP `wxwright_export` 让 CLI/Agent 也能拿到
  小红书文案 / 知乎 Markdown（此前只有 GUI 能切平台）；`PlatformSpec.export_kind` 是
  该行为的唯一来源；`wxwright image fit --platform <id>` 把文章图集裁到目标平台
  主形态（`PlatformSpec::image_profile()`：富文本平台只钳宽、图片平台取首预设 cover）。
- 发布绑定接口（social.rs）：X/LinkedIn 一键登录实装（PKCE/code flow + 本地回环监听 8761/8762 +
  keyring 存储，BYO 无云服务）；Facebook/Instagram 仅凭据接口（登录流受审核墙/图床前置所限，
  可行性见 docs/social-publish-oauth-feasibility.md）；配置引导帮助中心 = 顶栏问号（HELP_CONTENT
  结构化双语，{zh,en} 对构造性保证双语齐全）。已知未做：Meta/X/LinkedIn API 发布出口（凭据与登录
  已就绪，投递层未做）；demo 微信预览为静态 demo-preview.html（方言渲染在 Rust 引擎内，未在 JS 复刻）；
  AI 对话尚未迁移到 jobs.rs（有独立 harness，勿轻动）。

## 9. 改动后的必做清单 / Pre-delivery checklist

- [ ] `cargo test --workspace` 全绿；JS `node --check` 全过
- [ ] 前端改动 → cache-bust v+1
- [ ] 新 UI 字符串 → zh+en 双语 + data-i18n（四种属性机制）——`gui_governance_test` 会自动查
- [ ] 新 Tauri 命令 → lib.rs 注册 + async/spawn_blocking 检查（注册漏了 `gui_governance_test` 会红）
- [ ] 改过 JS → `node --check` 每个 `gui/ui/*.js`（CI 已加此门禁）
- [ ] 无 emoji 混入产品文件
- [ ] release 构建同步 dist + 重启 GUI
- [ ] CHANGELOG.md（Keep a Changelog 格式）+ 必要时 PRD §15 对照表
- [ ] 浏览器/桌面截图证据交给用户
- [ ] git commit（含新踩的坑 → 更新本文档第 7 节）
