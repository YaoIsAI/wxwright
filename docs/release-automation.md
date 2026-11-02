# Release automation (multi-platform build & publish)

发布流水线在 `.github/workflows/release.yml`：打 tag 或手动 dispatch 后，自动构建
全平台安装包、签名、生成更新清单，并挂到对应的 GitHub Release。

客户端在线更新的设计与排障见 `docs/auto-update.md`。

## 使用方式（发布一个版本）

```bash
# 1. 升版本（四处同步）
#    - 根 Cargo.toml 的 [workspace.package] version
#    - gui/src-tauri/tauri.conf.json 的 version
#    - gui/ui/app.js 的 GUI_VERSION
#    - cargo check（刷新 Cargo.lock）
# 2. 提交并打 tag
git tag v0.18.0
git push origin v0.18.0
# 3. Actions 跑完后，Release 自动出现全部产物与 latest.json
```

### 方式 B：不本地打 tag（workflow_dispatch）

Actions 页 → Release → Run workflow → 输入版本号（不带 v）。`guard-version`
会核对输入与 workspace `Cargo.toml` 一致，然后构建并发布。适合纯网页操作。

## 产物矩阵

| 平台 | 产物 | 说明 |
|---|---|---|
| Windows | `wxwright_<ver>_x64-setup.exe` + `.sig` | NSIS，`currentUser` 安装，双语；同时是更新载荷 |
| macOS | `wxwright_<ver>_<arch>.dmg`、`wxwright_<arch>.app.tar.gz` + `.sig` | aarch64 与 x86_64 两条；dmg 供首次安装，app.tar.gz 供更新 |
| Linux | `.deb`、`.AppImage` 及 `.sig` | deb 与 AppImage 都能在线更新（插件按安装类型选键） |
| 全平台 | `wxwright-cli-*.zip/tar.gz`、`SHA256SUMS.txt`、`latest.json` | CLI 四目标；清单用于客户端更新 |

## 实现要点

- **CLI**：四目标矩阵（windows-msvc / macos-arm64 / macos-x64 / linux-gnu），
  纯 `cargo build`，不需要 tauri。
- **GUI**：`cargo tauri build --bundles nsis|dmg,app|deb,appimage`
  （tauri-cli 经 taiki-e/install-action 安装；前端是纯静态 `ui/`，无需 Node）。
  Linux runner 需 `libwebkit2gtk-4.1-dev libxdo-dev
  libayatana-appindicator3-dev librsvg2-dev` 等依赖。
- **更新产物**：只有配置了 `TAURI_SIGNING_PRIVATE_KEY` 时，构建步骤才追加
  `--config src-tauri/tauri.updater.conf.json`（`createUpdaterArtifacts: true`），
  这样本地 `cargo tauri build` 不强制持有私钥。无密钥时打印 warning，发布
  不带 `latest.json`，客户端更新保持休眠。
- **签名**：`cargo tauri build` 用 minisign 私钥给 NSIS / AppImage /
  macOS app.tar.gz 生成 `.sig`。**deb 不在 bundler 的签名清单里**
  （`debian.rs` 没有 `sign_updaters` 调用），工作流随后用
  `cargo tauri signer sign` 以同一把 key 补签 `*.deb.sig`；CI 最后校验
  至少产出一个 `.sig`。
- **macOS 改名**：updater 归档固定叫 `wxwright.app.tar.gz`，两个架构会互相
  覆盖，构建后按架构改名为 `wxwright_aarch64.app.tar.gz` /
  `wxwright_x86_64.app.tar.gz`（改名发生在签名之后，minisign 不校验文件名）。
- **清单**：`tools/make-latest-json.py` 汇总各平台 URL 与签名，交叉校验签名
  key id 与 `tauri.conf.json` 公钥，生成 `latest.json`；发布后再用 `gh api`
  确认它真的挂到了 Release 资产上。
- **发布**：`softprops/action-gh-release@v2`，`generate_release_notes: true`；
  权限最小化（只有 release job 有 `contents: write`）。

## 签名 secrets

| Secret | 用途 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | minisign 私钥全文（在线更新，必须与 `plugins.updater.pubkey` 配对） |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥口令 |
| `WINDOWS_PFX_BASE64` / `WINDOWS_PFX_PASSWORD` | Windows 安装包代码签名（休眠，见 `docs/code-signing.md`） |
| `APPLE_*` | macOS Developer ID 签名与公证（休眠，配齐即启用） |

## macOS 签名与公证

无 `APPLE_SIGNING_IDENTITY` 时 tauri-bundler 完全跳过 codesign，产物无密封，
Apple Silicon 上会被 Gatekeeper 判为「已损坏」。发布流水线已固定
`APPLE_SIGNING_IDENTITY: '-'` 做 ad-hoc 密封，并用
`codesign --verify --deep --strict` + `_CodeSignature/CodeResources` 存在性
做门禁。配置 `APPLE_CERTIFICATE` / `APPLE_ID` / `APPLE_TEAM_ID` 等 secrets
即升级为 Developer ID 全公证，流程零改动（见 `docs/code-signing.md`）。

## 已知边界

- Windows ARM（aarch64-pc-windows-msvc）预留：在矩阵加一行即可。
- 不构建 MSI：NSIS 已覆盖安装与更新，见 `docs/auto-update.md` 第 5 节。
- 发布使用 `GITHUB_TOKEN` 时不能再触发其他工作流（坑 53）；因此 tag 与发布
  在同一个工作流体内完成，不要另做「打 tag 触发 Release」的壳。
- 首次 Linux/AppImage 构建较慢（appimagetool 下载）；可按需加缓存。
