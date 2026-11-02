# u60-pro-dash 的 fork 维护方式

这是 [locez/u60-pro-dash](https://github.com/locez/u60-pro-dash)，上游是 [jesther-ai/open-u60-pro](https://github.com/jesther-ai/open-u60-pro)。本 fork 维护 Rust/Slint 机身屏幕、浏览器交互原型、Tailscale 子网网关，以及 Linux / 旧版 ADB 下的安装校验修复。

## 分支和远程

| 名称 | 用途 |
| --- | --- |
| `origin` | 你的 fork，SSH：`git@github.com:locez/u60-pro-dash.git` |
| `upstream` | 原仓库，SSH 拉取；本地禁止向它推送 |
| `main` | 已整合上游和自定义功能的稳定分支，跟踪 `origin/main` |
| `feat/...`、`fix/...` | 日常功能分支；完成检查后合入自己的 `main` |

使用 merge 同步上游，保留已经推送的历史。不要对公开的 `main` 定期 rebase 或强制推送；也不要用 GitHub 的「Discard commits」同步，那会丢弃 fork 自己的提交。独立功能尽量留在 `u60-screen/`、`local-tools/`，必要的 agent 改动集中在小范围内，以减少每次合并的冲突。

## 日常同步

在仓库根目录运行：

```bash
git switch main

# 先抓取并查看上游变化，不合并、不推送
bash scripts/sync-upstream.sh --check

# 将最新上游合到本地 main，完成后检查差异和运行测试
bash scripts/sync-upstream.sh

# 检查通过，再经 SSH 推送 main
git -c push.followTags=false push origin main
```

如果已经确认本次上游变动、希望同步后立即推送，可以使用 `bash scripts/sync-upstream.sh --push`。它只推送 `main`，不推送 tag，也不使用 force；这个选项不会替你运行所有项目测试。

脚本要求完整历史、位于 `main`，合并前工作区干净。先接收 `origin/main` 的快进更新，再合并 `upstream/main`；本地与 fork 分叉时停止，交由你检查。合并冲突时保留现场，解决后 `git add <文件>`、`git merge --continue`；放弃本次合并用 `git merge --abort`。合并失败不会推送。

当前电脑另外配置了等价快捷命令 `git sync-upstream`、`git sync-upstream --check` 和 `git sync-upstream --push`。

## 在新电脑克隆

```bash
git clone git@github.com:locez/u60-pro-dash.git
cd u60-pro-dash
git remote add upstream git@github.com:jesther-ai/open-u60-pro.git
git remote set-url --push upstream DISABLED
git config remote.pushDefault origin
git config branch.main.pushRemote origin
git config pull.ff only
git config push.followTags false
git config fetch.prune true
git config rerere.enabled true
git config rerere.autoupdate false
git config alias.sync-upstream '!bash scripts/sync-upstream.sh'
```

这些是当前仓库的配置，不会改全局 Git 设置。`rerere` 记住以前的冲突解决方式，后续遇到同样冲突可复用；仍要检查结果并手动暂存。新电脑还需要自己的 GitHub SSH 授权与提交身份；想隐藏个人邮箱，在 GitHub 邮箱设置中复制 noreply 地址，再 `git config user.email '你的 GitHub noreply 地址'`。

日常二开从最新 `main` 拉功能分支：

```bash
git switch -c feat/my-screen
# 修改、检查，然后只暂存相关文件
git add u60-screen/
git commit -m "feat(screen): describe the change"
git push -u origin feat/my-screen
```

需要 PR 时目标选自己的 `locez/u60-pro-dash:main`；想贡献原仓库时再单独向上游提 PR。

## 检查与功能边界

- Rust 改动遵循 `CONTRIBUTING.md`：格式、Clippy、相关测试和 ARM64 musl release 构建。网关相关至少运行 `cargo test --locked -p zte-agent tailscale::tests`；发布构建还要检查 section 总大小和 gzip 大小。
- 同步脚本可运行 `python3 scripts/tests/test_sync_upstream.py`，测试使用临时本地仓库，不访问 GitHub 或设备。
- 小屏预览：`bash u60-screen/preview.sh`，见 [预览说明](u60-screen/README.md)。浏览器的 76 个入口以 `03f86da` API 为设计基线；原生版本补充 WireGuard、agent OTA 和本地锁屏，共 79 个功能入口。真实接口接入范围见 [原生开发说明](u60-screen/NATIVE.md)。
- 网关的旧版布局部署与验证范围见 [工具说明](local-tools/tailscale-gateway/README.md)。同步 Git、编译和推送不会更新真实设备。
- `setup.sh` 下载上游二进制的选项仍指向官方 release；需要 fork 功能时选择从本 fork 源码构建，不能用官方二进制代替。
- 上游新增的 OTA 使用签名发布；普通本地构建没有签名公钥，OTA 不可用。将来给 fork 发布 OTA 前，还需配置自己的发布地址、签名密钥和分支流程。本次只推送源码分支，不推送 release tag。

## 本次整合的检查记录

合入上游 `72d75ff` 后，16 项安装 / 网关保护 / 同步流程测试、12 项 Tailscale 单元测试、ARM64 musl release 构建均通过。该构建的 section 总大小为 6,873,752 字节，gzip（level 9、mtime 0）为 3,323,089 字节。小屏交互与动态配色已在本地 Chrome 验证。

Clippy 完成检查，现有上游代码有告警，本次修改的 Rust 行没有新增诊断。整个仓库的默认 `cargo fmt --all -- --check` 尚不通过；在未修改的上游文件上也能复现格式差异。本次保留上游风格，未对整个仓库批量格式化。

Gitleaks 对本次新增提交和待提交文件的扫描没有发现秘密；另人工检查了个人路径、设备记录、图片和构建产物。真实设备上的备份、登录身份与私密配置不属于推送内容。

## 提交前检查私密数据

```bash
git status --short
git diff --cached --stat
git diff --cached --check
git diff --cached
```

不要提交真实密码、API / Tailscale 令牌、SSH 私钥、Tailscale state、设备备份或导出的启动脚本。`.gitignore` 已忽略常见私密文件和网关构建产物，但它不能识别嵌在源码中的秘密，也不能撤回已跟踪的文件。示例保持虚构，命令使用相对路径，设备日志不直接复制进仓库。

可用 Gitleaks 在本机补充检查，例如推送前扫描 `gitleaks git --redact --log-opts='origin/main..HEAD'`。扫描发现真实秘密时先从待推送的所有提交中移除；不要仅在下一次提交删除它。如果已经公开，应立即撤销或轮换该秘密。
