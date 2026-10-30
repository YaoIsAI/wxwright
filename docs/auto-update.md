# 在线更新与 Windows NSIS 安装包

本文记录 wxwright 的客户端在线更新设计、签名发布流程，以及 Windows 发行形态的
研究结论。实现位置：

- 客户端：`gui/src-tauri/src/updater.rs`（两个命令）+ `gui/ui/app.js`（右下角入口）
- 发布侧：`.github/workflows/release.yml` + `tools/make-latest-json.py`
- 配置：`gui/src-tauri/tauri.conf.json` 的 `plugins.updater`

## 1. 结论（TL;DR）

- **Windows 发行形态就是 NSIS 安装包**（`wxwright_<version>_x64-setup.exe`），
  它同时是在线更新的载荷。Tauri v2 updater 原生支持 NSIS，不需要 MSI，也不
  需要额外维护一套 zip 更新包。
- **客户端内置更新**：右下角版本号就是入口。启动 3 秒后静默检查，之后每 6 小时
  一次；有新版本时版本号变成强调色按钮并带呼吸点，点击后直接下载、验签、安装。
- **安全模型**：`latest.json` 走 HTTPS；每个安装包都带 minisign 签名，客户端用
  编译进二进制的公钥验签，验签失败绝不执行。
- **无密钥也能发版**：没有配置签名 secret 时，发布流水线照常产出普通安装包，
  只是不生成 `latest.json`，客户端更新保持休眠，不会出现「有更新但装不上」。

## 2. 客户端行为

### 2.1 入口与状态

`index.html` 底部状态栏的 `#status-meta` 是一个按钮，默认显示
`wxwright <当前版本>`。`app.js` 的 `initUpdateUI()` 负责：

| 时机 | 行为 |
|---|---|
| 启动 3 秒后 | 静默调用 `update_check`，失败只写 console |
| 每 6 小时 | 再次静默检查 |
| 点击版本号（无可用更新） | 手动检查，并 toast「已是最新版本」 |
| 点击版本号（有可用更新） | 调用 `update_install`，进入下载 |
| 下载中 | 版本号位置显示 `正在下载更新 N%`，按钮禁用 |
| 安装中 | 显示 `正在安装，wxwright 即将重启…` |
| 失败 | 按错误类别 toast：网络 / 清单 / 签名 / 安装 / 其他 |

有可用更新时按钮加 `.has-update`（accent 底色 + 呼吸点），title 里带
`latest.json` 的 `notes`。UI 不新增任何对话框或独立停止按钮（AGENTS.md 第 5 节
的交互规范）。

### 2.2 各平台安装语义

| 平台 | 下载物 | 安装方式 | 重启 |
|---|---|---|---|
| Windows | `*-setup.exe`（NSIS） | 插件以 `/P /UPDATE /R` 启动安装器，被动模式只显示进度条，按当前用户安装 | 安装器 `/R` 自动拉起新版本；插件随后退出当前进程 |
| macOS | `wxwright_<arch>.app.tar.gz` | 解包替换 `.app` | 命令返回后调用 `app.restart()` |
| Linux（AppImage） | `*.AppImage` | 原位替换当前 AppImage | 命令返回后调用 `app.restart()` |
| Linux（deb） | `*.deb` | 通过 pkexec / 图形 sudo / 终端 sudo 执行 `dpkg -i` | 同上 |

**便携 exe 注意**：NSIS 的 `installMode` 是 `currentUser`，默认安装目录为
`%LOCALAPPDATA%\wxwright`。如果用户直接运行 `dist\wxwright-gui.exe` 并点击更新，
得到的是一份安装版（快捷方式与卸载项随之出现），原便携 exe 不会被原地替换。
需要保持绿色便携的用户请到 Releases 手动下载新 exe。

### 2.3 后端命令

`gui/src-tauri/src/updater.rs`：

- `update_check`：`updater_builder().timeout(20s).build()?.check()`，返回
  `{available, current_version, version, notes, date}`。错误统一返回
  `<类别>|<原文>`，前端按 `|` 前的类别翻译：`network`（连接失败）、
  `feed`（清单缺失、不是合法 JSON、或没有当前平台的条目）、
  `signature`（验签失败）、`install`（下载后安装阶段失败）、`other`。
  清单缺失单独成类，避免把「Release 还没带 latest.json」误报成代理问题。
- `update_install`：先重新检查一次（feed 可能已撤回），再
  `download_and_install(on_chunk, on_finish)`，进度通过
  `update-progress` 事件推送（`{downloaded, total}`），进入安装时推
  `update-installing`。用 `AtomicBool` 防重入。
- 插件默认启用 `system-proxy`，会读取系统/环境代理，避免再次踩到
  AGENTS.md 坑 38（`ureq` 不读代理导致境外端点全部超时）。

`updater.rs` 的命令同样受 `gui_governance_test` 守卫：`#[tauri::command]` 必须
出现在 `lib.rs` 的 `generate_handler!` 里。

## 3. 更新源与 `latest.json`

端点（`tauri.conf.json`）：

```
https://github.com/YaoIsAI/wxwright/releases/latest/download/latest.json
```

`/releases/latest` 自动指向最新的非预发布 Release，因此不需要每次改端点。
清单由 `tools/make-latest-json.py` 在发布时从产物目录生成：

```json
{
  "version": "0.17.0",
  "notes": "Release notes: https://github.com/YaoIsAI/wxwright/releases/tag/v0.17.0",
  "pub_date": "2026-10-30T12:00:00Z",
  "platforms": {
    "windows-x86_64": { "signature": "...", "url": "https://.../wxwright_0.17.0_x64-setup.exe" },
    "windows-x86_64-nsis": { "signature": "...", "url": "https://.../wxwright_0.17.0_x64-setup.exe" },
    "darwin-aarch64": { "signature": "...", "url": "https://.../wxwright_aarch64.app.tar.gz" },
    "darwin-x86_64": { "signature": "...", "url": "https://.../wxwright_x86_64.app.tar.gz" },
    "linux-x86_64": { "signature": "...", "url": "https://.../wxwright_0.17.0_amd64.AppImage" },
    "linux-x86_64-appimage": { "signature": "...", "url": "https://.../wxwright_0.17.0_amd64.AppImage" },
    "linux-x86_64-deb": { "signature": "...", "url": "https://.../wxwright_0.17.0_amd64.deb" }
  }
}
```

平台的查找顺序是 `{os}-{arch}-{installer}` → `{os}-{arch}`（插件源码
`get_urls`）。因此：

- NSIS 安装的 Windows 先查 `windows-x86_64-nsis`，便携 exe 直接用
  `windows-x86_64`，两者指向同一个 setup.exe；
- deb 安装的 Linux 先命中 `linux-x86_64-deb`，不会拿到 AppImage；
- AppImage 命中 `linux-x86_64-appimage`（generic 键兜底）；
- macOS 的 installer 名是 `app`，generic 键即可。

脚本还会做两件事：

1. **key id 交叉校验**：从 `latest.json` 提交的签名和
   `gui/src-tauri/tauri.conf.json` 的 `pubkey` 中解出 minisign key id，
   不一致直接失败。客户端用的是编译期公钥，发布端用错私钥的话所有用户都会
   拒装，这个检查把事故挡在发布前。
2. **完整性检查**：只要产物目录里出现任意 `.sig`，就必须四个平台齐全且每个
   产物都有 `.sig`，否则失败，避免「半个平台能更新」。

## 4. 密钥与 GitHub Secrets

私钥与口令只保存在仓库外的本机 `%USERPROFILE%\.tauri\` 下，不得入库、不得写进
文档或日志（AGENTS.md 铁律 6）。

```bash
# 生成（只需一次；-w 指向仓库外的路径）
cargo tauri signer generate -w "$USERPROFILE/.tauri/wxwright.key"
cat "$USERPROFILE/.tauri/wxwright.key.pub"   # 内容即 tauri.conf.json 的 pubkey
```

GitHub 仓库 Settings → Secrets and variables → Actions 添加：

| Secret | 值 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | `wxwright.key` 文件全文（base64 文本） |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 生成时设置的口令 |

`tauri.conf.json` 的 `plugins.updater.pubkey` 必须是**同一对**密钥的公钥。
CI 构建时 `cargo tauri build` 会打印 keynum 不匹配的警告，发布脚本会直接失败。

### 密钥轮换（两步走）

1. 生成新密钥对，把 `tauri.conf.json` 的 `pubkey` 换成新公钥；**这次发布仍用
   旧私钥签名**（用 `vars.UPDATER_PUBKEY` 指向旧公钥，供发布脚本交叉校验）。
   旧客户端只认旧公钥，所以能顺利升级到这一版；升级后它们开始认新公钥。
2. 从下一版起改用新私钥签名，并移除 `UPDATER_PUBKEY` 变量。

旧私钥在确认绝大多数用户已跨过过渡版本后即可废弃。若跳过过渡直接换钥匙，
旧客户端会因验签失败而无法自动更新，只能手动下载。

## 5. Windows NSIS 研究结论

### 5.1 为什么是 NSIS

- Tauri 2 的 Windows bundler 同时支持 NSIS 与 WiX/MSI；updater 对两者都支持。
- NSIS 体积小、默认 `currentUser` 安装（不需要管理员）、支持简体中文/英文
  双语界面，和「下载即装」的桌面工具定位一致。
- MSI 需要 WiX 工具链、安装体验更重，本项目没有企业分发/组策略需求，因此
  只构建 NSIS（`--bundles nsis`），不构建 MSI。
- 与 cc-switch（Tauri）同类方案；Magpie（Velopack）是另一套自成体系的更新器，
  迁移成本高且会引入 .NET 工具链，不采用。

### 5.2 当前 NSIS 配置

```json
"bundle": {
  "windows": {
    "nsis": {
      "installMode": "currentUser",
      "languages": ["SimpChinese", "English"],
      "installerIcon": "icons/icon.ico"
    }
  }
}
```

```json
"plugins": {
  "updater": {
    "endpoints": ["https://github.com/YaoIsAI/wxwright/releases/latest/download/latest.json"],
    "pubkey": "<minisign 公钥>",
    "windows": { "installMode": "passive" }
  }
}
```

- `bundle.windows.nsis.installMode = currentUser`：安装到
  `%LOCALAPPDATA%\wxwright`，无需 UAC 提权。
- `plugins.updater.windows.installMode = passive`：更新时 NSIS 只显示进度条，
  自动重启，不弹安装向导。
- 安装语言：系统语言是简体中文/英文时自动匹配；其他语言回退英文。
- WebView2 采用 Tauri 默认策略（下载 bootstrapper，无需管理员）；离线环境可
  自行改为 `offlineInstaller`，代价是安装包明显变大。
- 未配置代码签名时，SmartScreen 会提示「未知发布者」；签名流程见
  `docs/code-signing.md`（休眠，配置 PFX 即启用）。

### 5.3 产物命名

| 文件 | 来源 |
|---|---|
| `wxwright_<version>_x64-setup.exe` | NSIS 安装包，也是更新载荷 |
| `wxwright_<version>_x64-setup.exe.sig` | 对应的 minisign 签名 |
| `wxwright_<arch>.app.tar.gz` / `.sig` | macOS 更新包（CI 按架构改名） |
| `wxwright_<version>_amd64.AppImage` / `.deb` 及 `.sig` | Linux（deb 的 `.sig` 由工作流补签，bundler 不签 deb） |
| `latest.json` | 更新清单，Release 资产之一 |

## 6. 发布检查清单

```bash
# 1. 升版本（四处）：workspace Cargo.toml、tauri.conf.json、app.js GUI_VERSION、
#    Cargo.lock（cargo check 自动刷新）
# 2. 本地验证
cargo test --workspace
cargo fmt --all -- --check
node --check gui/ui/app.js
# 3. 提交后打 tag 或走 Actions 的 Release 工作流
git tag v0.17.0 && git push origin v0.17.0
```

发布完成后在本地复核：

```bash
curl -sL https://github.com/YaoIsAI/wxwright/releases/latest/download/latest.json | python -m json.tool
# 版本号应等于新 tag；每个 url 应能 302 到资产
```

客户端复核：启动已安装的旧版本，3 秒内右下角版本号应变成
`wxwright 0.16.0 → 0.17.0`；点击后应显示下载百分比并自动重启。

### 6.1 本地联调更新链路（不依赖 GitHub）

用一个本地 feed 验证「检查 → 显示更新按钮 → 下载 → 验签 → 安装报错」整条链路
（`TAURI_CONFIG` 只在本次构建生效，**发布构建绝不能带这个覆盖**）：

```bash
# /tmp/wx-feed/latest.json 里放 0.18.0 与 windows-x86_64-nsis 条目，
# 载荷用 `cargo tauri signer sign -f <私钥> <文件>` 签名
python -m http.server 8899 -d /tmp/wx-feed
TAURI_CONFIG='{"plugins":{"updater":{"endpoints":["http://127.0.0.1:8899/latest.json"],"dangerousInsecureTransportProtocol":true}}}' \
  CARGO_TARGET_DIR=E:/wxwright-build cargo build --release -p wxwright-gui
```

便携 exe 没有 NSIS 标记，插件只查 `windows-x86_64`，所以本地 feed 必须同时
提供该基键（正式清单由脚本自动补）。把载荷换成一个「签名合法但非 PE」的文件，
点击更新应依次走完下载与验签，最后以 `install|invalid updater binary format`
收尾且客户端不退出；篡改签名字节则应返回 `signature|The signature verification failed`。

## 7. 排障

| 现象 | 可能原因 | 处理 |
|---|---|---|
| 一直显示「已是最新」 | `latest.json` 未上传 / `version` 不大于当前版本 / Release 被标为预发布 | 检查 Release 资产；`/releases/latest` 会跳过预发布 |
| 提示网络错误 | GitHub 被代理或网络拦截；系统代理未生效 | 检查 `HTTPS_PROXY`/`ALL_PROXY`；插件走系统代理 |
| 提示没有有效更新清单 | 最新 Release 未附带 `latest.json`（未配签名 secret 的版本）或清单里没有当前平台键 | 按第 6 节发一版带签名的 Release；不要把它当网络故障 |
| 提示签名失败 | 发布私钥与客户端公钥不是一对；发布端用了新钥匙但客户端还是旧公钥 | 按第 4 节轮换流程处理；临时让用户手动下载 |
| Windows 安装器没弹出 | 安全软件拦截临时目录 exe；安装模式不是 passive | 查 `%TEMP%` 下 `*-installer.exe`；确认 installMode |
| macOS 更新后打不开 | dmg/app 未密封（历史问题） | 见坑 49；`codesign --verify --deep --strict` |
| 发布 job 红在 key id | 签名私钥与 `tauri.conf.json` 公钥不匹配 | 换回正确 secret，或按轮换流程设置 `UPDATER_PUBKEY` |

## 8. 已知边界

- `requireSignedVersion` 保持 `false`：本机 tauri-cli 2.11.4 生成的签名
  trusted comment 只有 `timestamp` 与 `file`，没有版本号；开启后所有更新都会被
  拒。代价是能篡改 `latest.json` 的攻击者可以把「更高版本号」指向一个旧的、
  签名合法的新版本（降级）。发布源是 GitHub HTTPS，暂接受该风险；CI 升级到
  写入版本号的 tauri-cli 后可评估开启。
- Windows ARM64 暂不发布（与 CLI 矩阵一致）。
- 便携 exe 不在 Release 中提供，更新会装出一份安装版（见 2.2）。
- Linux 仅提供 AppImage / deb 更新；rpm 未构建。
