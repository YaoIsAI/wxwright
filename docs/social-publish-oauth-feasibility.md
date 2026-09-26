# 海外平台一键登录与发布出口可行性 / Social OAuth Publish Feasibility

- 调研日期：2026-09-26（三路并行深调研，全部引官方文档，逐条标注 URL）
- 问题：既然渠道预览已覆盖 Facebook / Instagram / X / LinkedIn，能否像「公众号 API 绑定」一样，
  用一键登录（OAuth）完成配置并把发布出口做深？
- 结论先行：**可行，且四个平台的难度完全不同**。推荐按 X → LinkedIn → Facebook → Instagram 分四期落地，
  配置 UX 复用现有「公众号 API 绑定」的钥匙串模式。

## 0. 结论速览

| 平台 | 一键登录 | 个人发布 | 审核成本 | API 费用 | 核心阻断点 / 摩擦 |
|---|---|---|---|---|---|
| X (Twitter) | 可以，OAuth 2.0 PKCE 公共客户端，桌面最友好 | 可以（文本/图） | 零（自助注册，无审核） | 2026-02 起按量计费：纯文本 $0.015/条，**含链接 $0.20/条** | 无免费层（Public Utility Apps 除外）；redirect 端口必须精确匹配 |
| LinkedIn | 可以，3-legged OAuth | 可以（文本 3000 字 / 图） | **零**（Share on LinkedIn 是 Open Permission，即点即用） | 免费 | 文档要求 redirect 为 HTTPS（localhost 回环未文档化，需实测）；令牌 60 天且无 refresh token，要重授权 UX |
| Facebook Page | 可以 | 可以（feed 文本 + photos 图文） | 重：任意用户需 Business Verification + App Review + 年度 Data Use Check；开发模式手动加 tester 则零审核 | 免费 | 长期 Page token 换取需 client_secret（官方建议服务端执行，桌面端二进制内置 secret 与其指导相悖） |
| Instagram | 可以（Instagram Login 路径，无需绑定 FB Page） | **只能发媒体**，纯文本无 API 路径；限专业账户 | 同 Meta 审核墙 | 免费 | **图片必须是公网可达 URL**（本地文件发不了，需图床/中转方案）；60 天 token 需刷新 |

知乎/小红书不在本档范围：知乎无个人发文 API；小红书开放平台内容 API 资质受限——继续用现有
剪贴板/纯文本导出物。

## 1. 现状与可复用资产

- core `PlatformSpec.api_publish` 标志已存在（platform.rs，当前仅 wechat 为 true）——发布出口的
  分层开关位是现成的。
- 「渠道三件套」里导出物已按平台分化：X/LinkedIn/Facebook = caption 文本，Instagram = 图组，
  知乎 = Markdown（`platform_export_text`）。发布出口就是在这份导出物之上加一层 API 投递。
- 凭据存储：`wxwright-mp` 的 Credentials + keyring（`keyring::Entry::new("wxwright", ...)`，失败回退
  %APPDATA%）+ commands.rs `wx_bind / wx_bind_status / wx_unbind` 三命令——一键登录需要的新命令
  完全可以照这个模式长出来。
- 设置弹窗已是四分区导航（Providers / 公众号 API / ComfyUI / 关于），加「发布绑定」区是 UI 顺延。

## 2. 逐平台调研细节

### 2.1 X (Twitter) —— 桌面最友好，按量计费是唯一意外

- OAuth 2.0 Authorization Code + PKCE：authorize `https://x.com/i/oauth2/authorize`，token
  `https://api.x.com/2/oauth2/token`。**Native App = public client，官方明确不签发也不需要
  client_secret**——桌面应用的理想形态。
- 令牌：access token 2 小时；请求 `offline.access` 作用域即发 refresh token（官方未写明 refresh
  token 寿命），`grant_type=refresh_token` 静默续期。发帖作用域 `tweet.read tweet.write users.read`
  （传图加 `media.write`）。
- 回环重定向：`http://127.0.0.1:<port>` 被门户接受是**社区实证而非官方文档**（官方只说 exact match），
  端口必须与注册完全一致、不支持临时端口——应用内固定端口监听即可。
- 发帖：`POST /2/tweets`（文本必填 text）；280 字符上限跟随发帖用户订阅（Premium 可 25000）；
  媒体 v2 `POST /2/media/upload`（单发或 initialize/append/finalize 分块）。
- 费用（2026-02-06 起 Pay-Per-Use，console.x.com 充值）：Post Create $0.015/次；**含 URL 的帖子
  $0.200/次**（差 13 倍）；媒体上传端点未出现在计费表。旧 Free/Basic/Pro 分层已停售（Public
  Utility Apps 例外）。速率限制 100 次/15 分钟/用户。
- 限制：引用转推 API 需要 Enterprise；Following/Likes/Quote-Posts 已从所有自助档位移除（2026-04-16
  changelog）。

### 2.2 LinkedIn —— 审核成本为零的个人发帖

- **「Share on LinkedIn」（w_member_social）与「Sign In with LinkedIn using OpenID Connect」都是
  Open Permission**：开发者门户 Products 页点击即加，无审核、无合作伙伴计划、即点即用。
- OAuth 3-legged：authorize `https://www.linkedin.com/oauth/v2/authorization`（scope 必填），token
  `https://www.linkedin.com/oauth/v2/accessToken`，code 30 分钟有效。
- 发帖：`POST https://api.linkedin.com/rest/posts`，头部 `Linkedin-Version: 202609` +
  `X-Restli-Protocol-Version: 2.0.0`；纯文本帖官方支持，commentary 上限 3000 字符；author 为
  `urn:li:person:{id}`（id 取自 `/v2/userinfo` 的 `sub`，需实测与 `/v2/me` 一致性）。
- 图片：`POST /rest/images?action=initializeUpload` 返回 uploadUrl → PUT 二进制 → 帖子引用
  `urn:li:image:{id}`；`w_member_social` 即够（但只写，GET 不可用）。
- 令牌：60 天，**自助应用没有 refresh token**——官方替代是「静默重授权」（用户仍登录着 linkedin.com
  就跳过同意页），UX 上需要约第 55 天的重授权提醒。
- 回环重定向：文档写 HTTPS；`http://localhost` 是否可注册未文档化，需在开发者门户实测（保存时即知）。
- 公司主页发文需要 Community Management API 申请（仅限注册法人组织 + 企业邮箱 + Page 管理员验证，
  开发档 500 次/天）——第一版不做。
- 速率限制未公开具体数（仅开发档数字官方），429 按 UTC 零点重置。

### 2.3 Facebook Page —— API 成熟，审核墙最重，令牌策略最优

- 发帖：`POST /{page-id}/feed`（message/link）+ `POST /{page-id}/photos`（caption；单图）或多图
  先 `published=false` 传图再 `/{page-id}/feed` 带 `attached_media`。需 **Page access token** +
  `pages_manage_posts` + `pages_read_engagement` + `pages_show_list`，操作者需 Page 的
  CREATE_CONTENT 任务权限。
- 令牌：**长期 Page token 无过期时间**（用长期 user token 调 `/{user-id}/accounts` 换取）——一次
  登录终身可用，是四平台里令牌策略最好的；但换取过程需要 client_secret，官方文档明确要求这类
  交换「在服务端执行，绝不在客户端/应用二进制里」——纯桌面应用要么违背该指导内置 secret，
  要么加一个极小的令牌交换后端。
- 桌面 OAuth：官方桌面指引是 webview 内嵌 + `redirect_uri=https://www.facebook.com/connect/
  login_success.html` + `response_type=token`；Device Login（`/device/login`，用户访问
  facebook.com/device）仍受支持，但高级权限（pages_manage_posts）能否走设备流**未文档化**；
  localhost 回环无官方支持。
- 审核：开发模式把自己/测试用户加进 app 角色即可全权限发布（个人/小范围使用零成本）；**面向任意
  用户的 live 模式**需要 Business Verification + App Review（pages_manage_posts 等 Advanced
  Access）+ 年度 Data Use Check。个人开发者需创建 Business portfolio 并过企业验证，通过率无官方数据。
- 配额：Page 走 Business Use Case Rate Limits（4800 x 24h 参与用户数），无每帖上限；IG 配额见下。

### 2.4 Instagram —— 能做，但有一个硬阻断

- 两条路径 2026 并存：**Instagram API with Instagram Login**（`graph.instagram.com`，scope
  `instagram_business_basic` + `instagram_business_content_publish`，2025-01-27 起旧 scope 名废弃）
  **不需要绑定 Facebook Page**；旧路径（Facebook Login + Page token + `instagram_content_publish`）
  仍可用但要求 IG 专业账户关联 Page。
- 发帖两步流：`POST /{ig-user-id}/media`（创建容器）→ 轮询 `status_code` → `POST /{ig-user-id}/
  media_publish`。支持单图（**仅 JPEG**）、轮播（10 图）、Reels、Story；容器 24 小时过期。
- **硬阻断：所有容器创建都要求 `image_url`/`video_url`，即公网可达 URL——纯文本帖不存在 API 路径，
  本地文件也无法直接上传**。桌面应用要发 IG 必须先解决图床问题（候选：用户自填 URL；或社区常用的
  「先 unpublished 传到关联 FB Page 拿 CDN 链接再喂给 IG 容器」的 workaround——需实测验证）。
- 账户要求：Instagram 专业账户（Business/Creator）。
- 令牌：短效 1 小时 → 长效 60 天（`ig_exchange_token`）→ `ig_refresh_token` 续期（token 至少 24h
  龄）；同样标注服务端交换。
- 配额：50 帖/24h/账户（参考页机读值；指南页另有 100 的不一致表述，按 50 规划）。轮播算一帖。

## 3. 推荐架构（wxwright 怎么做）

### 3.1 配置模型：沿用「公众号绑定」心智，BYO（自带）应用凭据

每个海外平台 = 设置弹窗「发布绑定」区的一张卡：

- X / LinkedIn：填 `client_id`（X 为 Native App 公共客户端，无 secret；LinkedIn 另需 `client_secret`）
  + 「一键登录」按钮。**建议 BYO 而不是内置作者自己的 client_id**：①各平台用量与费用记在用户自己
  账户（X 按量计费尤其如此）；②避开「一个产品共享配额/费用/审核责任」的集中风险；③与公众号
  AppID/AppSecret 的心智完全一致。日后可加「作者托管模式」作为进阶选项。
- Facebook：填 `client_id` + `client_secret`，登录换取长期 Page token 后可弃用 secret（token 永不过期）。
- Instagram：依赖图床方案，最后做。

### 3.2 登录流：系统浏览器 + 本地回环监听（Tauri 标准做法）

1. 点「一键登录」→ Rust 侧起 `std::net::TcpListener` 绑定**每平台固定端口**（如 X 8761 /
   LinkedIn 8762 / FB 8763，冲突时给出明确报错），生成 `state`（CSRF）+ PKCE `code_verifier`。
2. `opener` 打开系统浏览器授权页（不在 webview 内嵌——X/LinkedIn 对嵌入式授权有风控与政策风险，
   系统浏览器也让用户复用已登录会话）。
3. 回跳 `http://127.0.0.1:<port>/callback?code=...&state=...` → 本地监听收码 → 校验 state →
   后端交换令牌（阻塞 IO 全部 spawn_blocking，铁律 1）。
4. 令牌入 keyring（复用 `wxwright-mp` 的 Credentials 模式，每平台独立 entry）：
   - X：refresh token（offline.access），2h access token 静默刷新。
   - LinkedIn：60 天 token，无 refresh——token 过期前 7 天在 UI 顶部给「重新授权」提醒（静默重授权
     不弹同意页，体验接近一键）。
   - FB：长期 Page token 永不过期；展示 Page 名称做绑定状态。
   - IG：60 天 token + `ig_refresh_token` 定期续期。
5. `wx_bind_status` 式的状态卡：已绑定账号/页面名、令牌剩余寿命、解绑按钮。

### 3.3 发布出口：在导出物上加一层投递

- 发布按钮出现在 `api_publish: true` 的平台 + 绑定状态下；「复制富文本」主按钮旁加「发布到 X /
  发布到 LinkedIn / 发布到 Page」次级动作（或导出菜单）。
- X：`platform_export_text` 产出的 caption 直接进 `text`；文章含外链时弹费用提示（$0.20/条）；
  图组走 v2 media upload。
- LinkedIn：caption 进 `commentary`（客户端先按 3000 字截断/提示）；图走 images API。
- FB：caption 进 `message`，首图走 `/photos`+caption。
- 全部命令 async + spawn_blocking；失败时把平台 API 错误原文透出（含 429 的「次日再试」语义）。

### 3.4 分期路线图

| 期 | 内容 | 为什么这个顺序 |
|---|---|---|
| P0 | X：绑定卡 + PKCE 回环登录 + 纯文本发帖 | 唯一无审核、无 secret、桌面标准流俱全的平台；打通整个「绑定→发布」骨架 |
| P1 | LinkedIn：同骨架复用 + 60 天重授权 UX + 图片 | Open Permission 零审核；代码与 P0 共享回环监听/令牌存储 |
| P2 | Facebook Page：BYO app（先 tester 模式自用）+ 长期 Page token + feed/photos | 审核/企业验证是外部依赖，先让作者本人和测试用户用起来；secret 交换是否上小后端在此期决策 |
| P3 | Instagram：图床方案定型（公网 URL 是硬前置）+ 容器两步流 | 阻断点在图床不在 OAuth，最后解决 |

### 3.5 风险与未验证点（诚实清单）

1. X 的 `http://127.0.0.1` 回环与 LinkedIn 的 localhost 注册均为**社区实证、官方未文档化**——P0/P1
   第一件事就是在两家开发者门户实测（保存即验证）。
2. X 按量计费的最低充值额（社区传约 $5）未经官方确认；含链接帖 $0.20 需要在产品里做费用提示。
3. Meta secret 交换「必须在服务端」的指导与纯桌面应用矛盾——P2 需要决策：违背指导内置 secret
   （个人工具可接受）vs 加一个极小的令牌交换后端（多一个运维件）。
4. IG 图床 workaround（FB Page unpublished 图喂 IG 容器）是社区常见做法，未在本次调研中官方验证。
5. LinkedIn `/v2/userinfo` 的 `sub` 与 `/v2/me` id 是否同值未官方文档化，P1 实测。
6. 各平台自动化发文政策（尤其 X 对机器行为的标记）不在 API 可行性范围内，产品侧应保持「用户亲手
   点发布」的交互以降低风控风险。

## 4. 参考（官方）

- X：docs.x.com OAuth2 PKCE / create-post / media / pricing / changelog / rate-limits
- LinkedIn：learn.microsoft.com/en-us/linkedin —— posts-api / images-api / authorization-code-flow /
  getting-access（Open Permissions）/ community-management-app-review / rate-limits
- Meta：developers.facebook.com —— pages-api/posts / page/feed / page/photos / instagram-platform/
  content-publishing / instagram-api-with-instagram-login/business-login / facebook-login for devices /
  manual-flow / get-long-lived / release & business-verification / rate-limiting
