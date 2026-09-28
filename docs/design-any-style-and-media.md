# 设计稿：任意样式 + 图像管道 + AI 配图（2026-09-28）

对应三个产品目标：「AI 生成任何样式的文章」「图像跨平台裁剪匹配」「AI 生成内容时同步生图」。
原则：架构极简、数据驱动、一切判定只写一处、双向契约测试锁定。

## 1. 主题系统 v2 —— role × variant 二维表

**问题**：`Theme.blocks` 是「角色 → 属性」平表。同一角色在全文只有一种长相：
引用块无论是一句话金句还是三段长引文都是同一个底色；首段导语和普通正文同款。
AI 只能生成「每个角色一种样式」的主题，表达不了内容感知的版式。

**设计**：引入 variant 维，键编码为 `role.variant`（点分命名空间），**不改 Theme 结构、不改 IR**。

- TOML 形态（合法 TOML：子表与标量属性可共存于同一表）：

  ```toml
  [block.quote]
  background = "{quote_bg}"

  [block.quote.hero]        # 变体：短金句（单段 ≤ 40 字）独立成卡
  background = "none"
  border-left = "4px solid {accent}"
  font-size = "20px"
  ```

- 合并顺序（唯一实现处 = `render.rs::apply_role_overrides`）：
  引擎默认 → `blocks[role]` → `blocks["role.variant"]`（后者逐属性覆盖前者）。
  leaf 同理：`role.variant_leaf` 覆盖 `role_leaf`。
- 变体判定在渲染期从内容计算（即「内容分类 pass」，纯函数 + 一个 Cell 状态位）：
  - `quote.hero`：blockquote 恰含一个段落且去空白字符数 ≤ 40；
  - `paragraph.lead`：全文渲染的第一段（导语位）。
  不把 variant 写进 IR：变体是内容局部属性，渲染期可判定；跨块上下文走 Ctx
  状态位。若未来出现必须全局分析的变体，再升级为独立 pass，接口不变。
- `roles.rs` 是唯一事实源：`RoleDef.variants: &[&str]`，key 展开规则随 shape
  叠加 `role.variant` / `role.variant_leaf`，AI 提示词的变体清单从表生成
  （`prompt_variant_list()`），`is_known_key` 覆盖点分键。
- v1 变体集合刻意小：`quote.hero`、`paragraph.lead` 两个家族。变体是给
  「内容形态」用的，不是给「换个颜色」用的——那是角色的职责。

**测试契约（双向）**：声明了 → 探针文档渲染后必须出现（theme_roles_test 扩展）；
渲染器会产生 → roles 表必须已声明（key 数算术 + 变体展开一致性测试）。
作用域双向：短金句吃到 hero、长引用必须吃不到。

## 2. 图像跨平台裁剪匹配 —— 消费已有的平台画像

**问题**：img.rs 只有 resolve（尺寸读数/内联/上传），零像素处理。文章里的
原图直接扔给平台：3:2 横图上小红书封面（要 3:4）被硬裁，1080 宽规则无人把守。

**设计**：`ImageProfile { max_width, cover_aspect }`，从 `PlatformSpec.presets`
（平台画像表，本来就是数据）推导，不新增配置面：

- 图片笔记平台取首预设为封面形态：xhs = 1080×1440(3:4)、instagram = 4:5、
  X/LinkedIn = 1:1；富文本平台（wechat/zhihu）= max_width 1080、不裁。
- `img.rs::fit_to_profile`：居中 cover 裁切到目标比例 → 宽度只降不升钳到
  max_width。PNG 保 PNG、其余编码 JPEG q85。同一函数服务所有出口。
- CLI `wxwright images --platform xhs article.md --out-dir D`：从文章提取
  本地图集 → 逐张按平台画像适配 → 输出 `01.jpg…` + JSON 清单。人机两用，
  MCP 后续直接包一层（工具面已锁定，扩面需同步 agent card，单独走）。

**测试契约**：对程序生成的纯色图断言输出尺寸=裁切公式（横图 3:2 → 3:4 输出
960×1280 之类）；宽度钳制只降不升；PNG 透明度保留。

## 3. AI 同步生图 —— 「illustrate」复合 job

**问题**：文字生成（theme/svg/poster/chat）与图片生成（image/comfy）是分离的
job，写完文章要手动去绘图面板想提示词、传图、插图——「一键创作」断在中间。

**设计**：jobs.rs 新增 kind `illustrate`（铁律：一切 AI 生成走 jobs.rs，事件
协议不变 `{id, kind, ev, data}`），参数 `{ path, count, platform }`：

1. 读文章 → 一次性文本调用（现有 `call_completions_post`，JSON 提取沿用
   预算阶梯）：产出 1–3 个插图位 `{anchor(锚点段落前缀), alt, prompt}`；
2. 逐位调 `ai::generate_image`（现有云端生图，模型/密钥逻辑零复用成本）→
   `fit_to_profile` 按目标平台画像适配（wechat 默认 1080 不裁）；
3. 落盘到文章同目录（`<stem>-illust-N.jpg`），在锚点段落后插入
   `![alt](绝对路径)`，写回文章；事件透出每步 status。
4. 前端复用 ai-jobs.js 按钮态（生成中 → 停止），不新增控制台框（铁律），
   完成后重载文章 + 刷新预览。

**测试契约**：锚点匹配失败 = 明确 error 而非静默跳过；count 钳制 1..=3；
取消传播到图片调用之间。

## 交付顺序

1 → 2 → 3，各自独立 commit、测试全绿后交付 dist。移动端闸门（公众号助手
真机显示 API 草稿排版）需作者真机验证，不在此列。
