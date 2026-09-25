# Release automation (multi-platform build & publish)

评估结论：**完全可行，脚本已预备好**（`.github/workflows/release.yml`）。发布到 GitHub 后，打 tag 即自动产出全平台安装文件并挂到对应 Release。

## 使用方式（发布一个版本）

```bash
# 1. 升版本（workspace Cargo.toml 的 [workspace.package] version + gui/src-tauri/tauri.conf.json 的 version）
# 2. 提交并打 tag
git tag v1.0.1
git push origin v1.0.1
# 3. Actions 跑完后，Release 页面自动出现全部产物（generate_release_notes 自动写更新说明）
```

## 产物矩阵

| 平台 | 产物 | 说明 |
|---|---|---|
| Windows | `wxwright_*_x64-setup.exe`（NSIS 安装包）+ 便携 `wxwright-gui.exe` + `wxwright-cli-windows-x64.zip` | NSIS 由 tauri bundle 生成 |
| macOS | `.dmg` + `.app.tar.gz`（universal：x86_64 + aarch64 经 tauri universal target 或双构建）+ CLI arm64/x64 tar.gz | **未签名**；签名/公证见下 |
| Linux | `.deb` + `.AppImage` + 静态 `wxwright-cli-linux-x64.tar.gz`（musl） | runner 装 webkit2gtk-4.1 等 |
| 全平台 | `SHA256SUMS.txt` | 发布前自动生成 |

## 实现要点（与 PRD 12.3 对齐）

- **CLI**：四目标交叉矩阵（windows-msvc / macos-arm64 / macos-x64 / linux-musl 静态），纯 `cargo build`，无 tauri 依赖。
- **GUI**：`cargo tauri build --bundles <nsis|dmg,app|deb,appimage>`（tauri-cli 经 taiki-e/install-action 安装，不需要 Node——前端是纯静态 `ui/`）。Linux runner 需 `libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev`。
- **发布**：`softprops/action-gh-release@v2`，`generate_release_notes: true`；权限最小化（`contents: write`）。
- 版本号单一来源：workspace `Cargo.toml`（tauri.conf.json 需同步，未来可加 CI 校验步骤）。

## macOS 签名与公证（可选，需要 Apple 开发者账号）

未签名 dmg 会被 Gatekeeper 拦（右键打开可绕过）。要正式分发，在 GitHub Secrets 配置：

```
APPLE_CERTIFICATE / APPLE_CERTIFICATE_PASSWORD / APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID
```

然后在 build-gui 的 macOS job 里加 tauri 官方签名环境变量与 `tauri build --bundles dmg`（tauri-cli 自动调用 notarytool）。PRD 12.3 的"Developer ID 签名 + notarize"由此满足。

## 已知边界

- Windows ARM（aarch64-pc-windows-msvc）预留：在 build-cli matrix 加一行即可（PRD v1.x 计划）。
- 更新检查不做常驻（PRD 3.2）：产物不含自动更新通道；如需 tauri updater，属后续 opt-in 功能。
- 首次跑 Linux/AppImage 构建较慢（appimagetool 下载）；可加缓存层。
