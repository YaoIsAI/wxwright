# 代码签名决策指南（Windows SmartScreen / macOS Gatekeeper）

> 给维护者的决策文档。当前状态：**全部产物未签名**——Windows 安装包首次运行
> 会被 SmartScreen 拦截（「Windows 已保护你的电脑」→ 需点「仍要运行」），
> macOS dmg 需右键 → 打开。本页把签名这件事变成一页可拍板的账。

## 为什么重要

未签名是普通用户安装的第一道墙：SmartScreen 对无信誉签名（包括新买的证书
前几周）都会拦，下载量/安装量积累信誉需要时间。**开源工具的用户习惯绕过，
普通用户不会**——要不要买签名 = 要不要认真做非开发者用户增长。

## Windows（SmartScreen）

| 证书类型 | 大致年费 | SmartScreen 立即绿 | 备注 |
|---|---|---|---|
| OV（组织验证） | ¥1500–3500/年 | 否，需积累信誉 2–4 周 | 主流选择；需企业或个人身份验证 |
| EV（扩展验证） | ¥3000–6000/年 | **是**（硬件令牌存储） | 立即生效；证书绑硬件 USB key 或云签名 |

- 供应商：Certum（开源友好，Open Source Developer 证书约 €25/年，个人可办）、
  SSL.com、Sectigo、DigiCert
- **开源项目性价比首选：Certum Open Source Code Signing**——专为开源作者设，
  便宜且被广泛认可
- 签名集成点：`release.yml` 的 build-gui (windows) job 产物上传前加签名 step
  （signtool sign /fd sha256 /tr <timestamp-url> /td sha256 <file>），密钥放
  GitHub Secrets

## macOS（Gatekeeper / notarization）

- 必须 Apple Developer Program（**$99/年**，个人即可）
- 流程：Developer ID Application 证书签名 → notarytool 公证 → staple
- 集成点：release.yml 的 build-gui (macos) job；证书（.p12）与 App 专用密码
  放 GitHub Secrets；`xcrun notarytool submit --keychain-profile`

## 建议路径

1. **先 Certum 开源证书**（€25/年）：Windows 侧投入最小、开源身份好办
2. macOS：若 Mac 用户占比可观再上 Apple Developer；否则维持「右键 → 打开」
   说明（README 已有）
3. 两者都可后置到「有真实用户反馈被拦」时再买——**证书会过期，早买不划算**

## 决策所需的一切都在这页。要动手时：告诉我买哪家，我把签名 step 写进
release.yml 并跑通一次签名发布。
