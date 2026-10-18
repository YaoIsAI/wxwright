# 图标 SOP / Icon Standard Operating Procedure

> 事故背景（v0.12.1，2026-10-18）：macOS dmg 的应用图标比系统标准图标**大一圈**——
> 因为把 Windows 风格的满幅方形图标直接放进了 icns。苹果官方规范要求图标画在
> 1024 画布中央 824px 的连续曲率圆角方（squircle）内、四周透明、带柔和投影。
> 本 SOP 固化生成与验证流程；管线已自动化（`tools/icongen`），照做即不会复发。

## 1. 平台规范对照（一张表记住）

| 平台 | 画布 | 构图 | 形状 | 产物 |
|---|---|---|---|---|
| Windows | 256 | **满幅**（顶到四边，任务栏惯例） | 圆角矩形 rx=225/1024 | `icon.ico`（8 帧，Lanczos） |
| Linux | 512 | 满幅 | 同上 | `icon.png` + 各尺寸 PNG |
| **macOS** | 1024 | **824/1024 内缩居中**（80.5%，四周透明） | **连续曲率 squircle**：超椭圆 `\|x/a\|⁵+\|y/a\|⁵=1`，a=412；**不是**圆角矩形 | `icon.icns`（icp4…ic10 十条目 + 柔和投影） |

为什么 macOS 特殊：Dock/Finder 按位图原样渲染图标格子，满幅位图视觉上比所有
标准图标大一圈；苹果自家与一线三方应用全部遵循 824 网格 + squircle + 投影。

## 2. 生成流程（唯一入口，禁止手改产物）

1. 只改 **`assets/icon/master.svg`**（1024 满幅设计稿；改 logo 需三处同步：
   master.svg、`gui/ui/index.html` symbol、重跑 icongen——见 AGENTS.md 铁律 10）。
2. 重跑生成器：

   ```bash
   cargo run --release -p icongen
   ```

   它一次产出：4 张 PNG + 8 帧 ICO（满幅，Win/Linux 用）+ **icon.icns
   （macOS 官方规格：824 squircle + 模板投影，自动合成）**。
3. `git diff` 检查 `gui/src-tauri/icons/`——产物只允许由 icongen 产生，
   任何手工 PS/预览软件的改动都是事故。

## 3. 提交前验证（命令级）

```bash
# icns 结构 + 几何合规（ic10 应为 1024 画布、四角透明、
# 内容跨度 = 824 tile + 投影晕染 ≈ 880-890）
python - <<'EOF'
import struct, io
from PIL import Image
raw = open("gui/src-tauri/icons/icon.icns","rb").read()
assert raw[:4] == b"icns" and struct.unpack(">I", raw[4:8])[0] == len(raw)
off = 8; entries = {}
while off < len(raw):
    t = raw[off:off+4].decode(); ln = struct.unpack(">I", raw[off+4:off+8])[0]
    entries[t] = raw[off+8:off+ln]; off += ln
img = Image.open(io.BytesIO(entries["ic10"])).convert("RGBA")
bbox = img.getchannel("A").getbbox()
print("entries:", len(entries), "bbox:", bbox, "span:", bbox[2]-bbox[0])
assert img.getpixel((5,5))[3] == 0, "corner must be transparent (padded)"
assert 840 <= bbox[2]-bbox[0] <= 900, "tile+shadow span out of macOS range"
print("ICON-SOP CHECK OK")
EOF

cargo fmt --all -- --check && cargo clippy -p icongen
```

## 4. 发版后终验（从真实 dmg 复验）

```bash
gh release download vX.Y.Z --pattern "*.dmg" --dir /tmp/icon-check
"/c/Program Files/7-Zip/7z.exe" x -y -o/tmp/icon-check/x /tmp/icon-check/wxwright_X.Y.Z_aarch64.dmg
# 用第 3 节同样的脚本校验
# /tmp/icon-check/x/wxwright/wxwright.app/Contents/Resources/icon.icns
```

## 5. 禁止事项（每条都对应一次真实事故）

1. **禁止**把满幅图标当 macOS 图标——824 网格 + squircle + 投影缺一不可
   （v0.12.1 及之前所有版本的事故）。
2. **禁止**手改 `gui/src-tauri/icons/` 下任何文件（含"只用 tauri icon 命令
   顺便重生成"——它会用满幅源覆盖 icns，且丢掉 8 帧 ICO；唯一入口是 icongen）。
3. **禁止**用普通圆角矩形替代 squircle——大尺寸下曲率差异肉眼可见。
4. 换图标配色/形状时，**先在 824 squircle 构图下预览**再定稿（满幅好看 ≠
   内缩后好看）。
