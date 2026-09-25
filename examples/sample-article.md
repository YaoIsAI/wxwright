# wxwright 一分钟上手

把 **Markdown** 交给 wxwright，一条命令得到符合微信公众平台规范的富文本，支持 *斜体*、~~删除线~~、`行内代码` 与[官方文档](https://mp.weixin.qq.com)链接。

## 为什么零失真

- 产出即合规：`section` 块级 + `span[leaf]` 行内，全内联样式
- 图片全部 mmbiz 化，或给出明确降级提示
- 内置校验器与官方 verify CLI 对齐
    - 规则覆盖 R-1 / R-2 / R-3 / R-4 全表
    - 每条规则都有违规样例测试

任务清单：

- [x] 写好 Markdown
- [x] `wxwright copy article.md`
- [ ] 打开公众号编辑器，Ctrl+V

> [!NOTE]
> 无需任何凭据即可使用 convert / validate / copy 三个命令。

> 这是一段普通引用，呈现为浅灰卡片样式。

> [!KEYPOINT] 引擎是纯 Rust 单二进制，冷启动毫秒级，常驻内存零。

| 能力 | 状态 | 说明 |
|:-----|:----:|------|
| 表格自适应 | 支持 | min-width 策略 |
| 代码高亮 | 支持 | 服务端着色全内联 |
| 公式 | 支持 | 文本卡片保真 |

```rust
fn main() {
    println!("hello 公众号");
}
```

质能方程 $E = mc^2$ 也可以直接内联。

---

1. 第一步：安装 wxwright
2. 第二步：写作并复制
3. 第三步：粘贴到公众号后台

> [!WARNING]
> 剪贴板模式要求图片为 mmbiz 或 https 直链，本地图片请先 `wxwright login`。
