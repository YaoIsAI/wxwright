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
  便宜且被广泛认可。两个必须知道的现实：
  1. **证书实名**：证书主体是开发者的法定姓名（验证 = 提交能证明身份的开源
     项目链接 + 实名认证），签名详情里用户看到的是真实姓名，不是笔名。
  2. **私钥不可导出**：私钥存 Certum 云端虚拟卡（SimplySign），不提供 PFX 文件，
     `WINDOWS_PFX_BASE64` 那条 CI 通道用不上。正确姿势是本地签名：装 SimplySign
     Desktop（虚拟卡出现在 Windows 证书库）→ `signtool sign /a /fd sha256
     /tr http://timestamp.digicert.com /td sha256 <文件>`，每次发版在本机一条
     命令签完 dist 产物再传 Release——私钥不进 GitHub，反而更安全。
- `release.yml` 里休眠的 PFX 签名 step 只适用于「能导出 PFX 的证书」；2023 年起
  CA/B 规范要求私钥进硬件，新购证书基本都给不了 PFX，该 step 保留仅为将来
  云签名 API 类方案预留。走 Certum 就本地签，不改 CI。

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

## 决策所需的一切都在这页。要动手时：告诉我买哪家——买 Certum 我把本地一键
签名脚本（签 dist 全部产物）写好，陪你跑通第一次；其余看证书形态再定集成方式。
