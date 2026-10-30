# Gitee 双仓同步 SOP（两套说明 / 两套推送标准）

> 适用仓库：主仓 `github.com/YaoIsAI/wxwright`，Gitee 镜像 `gitee.com/YaoIsAI/wxwright`。
> 镜像建立于 2026-10-21；凭证存 Windows 凭据管理器（Git Credential Manager），日常推送免密。
> 原则：**代码同一棵树，说明按站点分开写，推送与发版各有一套标准。**

## 0. 两套说明（文档分工）

| 文件 | 面向 | 约定 |
|---|---|---|
| `README.md` | GitHub 首页（英文） | 徽章、下载、克隆、作者链接全部指向 GitHub |
| `README.zh-CN.md` | GitHub 中文入口 | 同上，中文正文，顶部语言切换连到 `README.md` |
| `README_GITEE.md` | **Gitee 首页（中文）** | 徽章用 Gitee star/fork，下载指向 Gitee 发行版，克隆/作者指向 `gitee.com/YaoIsAI`；**不得出现 GitHub 链接**；在线更新需给出手动下载兜底 |

- Gitee 仓库首页默认展示 `README_GITEE.md`：仓库 → 管理 → 仓库设置 → 默认页面，
  选择该文件（Gitee 网页入口名称可能微调，以站点当前 UI 为准）。
- 三个 README 的**版本号、资产文件名、功能清单必须同步修改**；只有链接语义按站点不同。
- `README_GITEE.md` 里对在线更新的表述：内置更新源不可达时，到 Gitee 发行版手动
  下载覆盖安装；不承诺 Gitee 提供自动更新 feed（客户端只认内置签名源）。
- 仓库描述（Gitee 管理 → 基本信息）同样只写 Gitee 语义，不夹带 GitHub 链接。

## 1. 两套推送标准

### 标准 A：代码推送（每次提交）

`main` 是唯一分支，两个远端必须指向**同一个 commit**，不做 Gitee 专属分支、
不做内容分叉：

```bash
git push origin main
git push gitee main

# 校验同一棵树（两个 SHA 必须相同）
test "$(git rev-parse origin/main)" = "$(git rev-parse gitee/main)" || echo "MISMATCH"
```

标签同样双推：`git push origin --tags && git push gitee --tags`。
禁止对任一远端 `main` force push；出现 non-fast-forward 时先 `git fetch` 对齐。

### 标准 B：发行版（每次发版）

- GitHub：Release 工作流自动构建 + 签名 + 合成 `latest.json` + 发布（唯一自动
  更新源，客户端验签只认它）。
- Gitee：**手动镜像发行版**——同一 tag、同一批产物、同一份变更说明，附件逐个
  上传（见第 4 节）。Gitee 附件仅供手动下载，不参与客户端在线更新。
- 发版后核对两处：GitHub Release 资产齐全且含 `latest.json`；Gitee 发行版
  附件数量与文件名和 GitHub 一致。任何一边缺资产都算未完成。

## 2. 日常发版双推（GitHub 工作流打 tag 的场景）

任何一次 push 到 GitHub 之后（发版尤其），把 main 与标签镜像到 Gitee：

```bash
git push origin main

# 0.12.2 起的自动发版由 Release 工作流在 GitHub 上打 tag —— 先拉齐到本地：
git fetch origin --tags

# 再镜像到 Gitee：
git push gitee main
git push gitee --tags
```

注意：本地手动打的 tag 不需要 `git fetch origin --tags`；只有工作流打的标签需要先拉。

## 3. 仓库描述与设置

Gitee API v5 **只认私人令牌**；git 推送的账号密码不能调 API（已实测 401）。

- 方式 A（零密钥，推荐）：网页修改。仓库 → 管理 → 基本信息 → 描述。
  直达：https://gitee.com/YaoIsAI/wxwright/admin
- 方式 B（API 自动化）：先到 https://gitee.com/profile/personal_access_tokens
  建私人令牌（勾 `projects` 权限），存到本机**仓库外**的临时文件：

```bash
TOKEN=$(cat "$TEMP/gitee_token.txt")   # 令牌只进进程内存，不回显、不入文档
python -c "import json; open('desc.json','wb').write(json.dumps({'description': '这里替换为描述文案'}, ensure_ascii=False).encode('utf-8'))"
curl -s -X PATCH "https://gitee.com/api/v5/repos/YaoIsAI/wxwright?access_token=$TOKEN" \
  -H "Content-Type: application/json" --data-binary @desc.json
rm desc.json && rm "$TEMP/gitee_token.txt"   # 用完即删
```

**中文乱码防线**：含中文的 API payload 必须写成 UTF-8 文件再用
`curl --data-binary @file` 提交，不要在命令行里直接拼中文字符串
（Windows 控制台代码页会把 UTF-8 拼成乱码）。

## 4. 发行版（Releases）与附件

Gitee 的「发行版」就是 GitHub Release 的对应物：基于 tag + 标题 + 说明 +
附件（安装包挂这里）。镜像建立时只同步了 tags，发行版与附件默认为空。

API 流程（需要私人令牌，见第 3 节方式 B）：

```bash
# 1) 基于已有 tag 建发行版（release.json 为 UTF-8 文件，body 里写变更说明）
curl -s -X POST "https://gitee.com/api/v5/repos/YaoIsAI/wxwright/releases?access_token=$TOKEN" \
  -H "Content-Type: application/json" --data-binary @release.json
# release.json 示例：{"tag_name":"v0.16.0","name":"v0.16.0","body":"变更说明","prerelease":false}

# 2) 上传附件（上一步响应里有 release id）
curl -s -X POST "https://gitee.com/api/v5/repos/YaoIsAI/wxwright/releases/<RELEASE_ID>/attach_files?access_token=$TOKEN" \
  -F "file=@wxwright_0.16.0_x64-setup.exe"
```

附件有大小与配额限制（免费账户按文件/仓库空间计，以 Gitee 官方说明为准）。
每个版本约 10 个二进制，**全量回填前先权衡配额**——建议只回填最近几个版本，
或只放 CLI 便携包。

## 5. 安全红线（最高优先级，不可妥协）

1. 令牌/密码**绝不进入**：仓库、任何文档（文档里只允许 `<GITEE_TOKEN>`
   占位符）、聊天记录、日志、截图。
2. 令牌只允许存在两处：Windows 凭据管理器（git 推送用），或本机仓库外的
   临时文件（API 用，**用完即删**）。
3. 当前凭据管理器里存的是账号密码——只够 git 推送；调 API 必须另建
   「私人令牌」。
4. 令牌疑似泄露的应急动作：立即到
   https://gitee.com/profile/personal_access_tokens 删除该令牌并重建。
5. 提交前自查：`git log -p` / `git diff --cached` 中不得出现任何 token、
   secret、密码字面量；密钥类本地文件（`agnes ai.txt`、`settings.json`、
   `dist/`）保持在 .gitignore 排除状态。

## 6. 故障排查

| 现象 | 处置 |
|---|---|
| push 要求重新登录 | 凭据管理器条目失效：重走一次 GCM 授权弹窗即可 |
| API 401 | 令牌无效/过期/没勾 `projects` 权限：重建令牌 |
| push 报 non-fast-forward | Gitee 侧被网页改过：先 `git fetch gitee` 对齐；**禁止对 main force push** |
| 中文变乱码 | API payload 一律走 UTF-8 文件 + `--data-binary`，不走命令行拼接 |
| Gitee 首页仍显示旧 README | 检查「默认页面」是否指向 `README_GITEE.md`；确认文件已推送到 main |
