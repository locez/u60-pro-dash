# U60 热点访问家庭子网

这份配套工具让 U60 热点客户端通过 Tailscale 访问另一台子网路由器发布的家庭内网。当前规则固定为 U60 LAN `192.168.0.0/24` → 家庭子网 `192.168.1.0/24`，已在 U60 Pro B31、Tailscale 1.104.4 上验证。其他网段需要先修改并核对脚本中的地址，不能直接套用。

热点客户端无需安装 Tailscale；普通互联网流量继续走蜂窝网络。家庭子网路由必须在 Tailscale 后台批准，家庭节点保持在线，访问策略允许 U60 访问目标地址。

## 适用安装方式

`enable-u60-home.sh` / `deploy.py` **只支持旧版直启布局**：`/data/zte-agent` 是普通文件，`/data/local/tmp/start_zte_agent.sh` 直接启动 agent。脚本会在写入前拒绝 OTA 槽位布局，回滚也执行这一检查。

上游新版 `setup.sh` 已采用 A/B 槽位和 supervisor。不要用本工具替换它的 `/data/zte-agent` 符号链接；适配新布局的设备部署流程尚未实现。源码可以正常合并、编译和测试，不代表已验证新 OTA 布局的实机部署。

## 源码与构建产物

- `zte-agent/src/tailscale/mod.rs`：网关功能的维护入口，已经合入 fork；不再反复应用独立补丁。
- `u60-home-network.sh`：设备上的 IPv4 转发、NAT、TCP MSS 规则，只处理指定的两个网段。
- `enable-u60-home.sh`：Linux 电脑入口，依赖 `adb`、`python3` 和 USB 调试连接。
- `deploy.py`：环境检查、设备内备份、部署、连通性验证及失败回滚。
- `zte-agent`、`zte-agent.sha256`：本地构建产物，由下列命令生成，不提交到 Git。旧的 `upstream-gateway.patch` 也是本地产物，源码以 Git 提交为准。

从仓库根目录运行，先准备 Rust 的 `aarch64-unknown-linux-musl` 目标和对应交叉编译器（参见根目录 `CONTRIBUTING.md`）：

```bash
cargo build --locked --release --target aarch64-unknown-linux-musl -p zte-agent
cp target/aarch64-unknown-linux-musl/release/zte-agent local-tools/u60-home-gateway/zte-agent
(cd local-tools/u60-home-gateway && sha256sum zte-agent > zte-agent.sha256)
```

## 旧版布局的部署与回滚

已配置成功的设备无需重复部署。在仓库根目录运行：

```bash
# 只读检查；首次部署前应处于 upstream 默认的 userspace 模式
bash local-tools/u60-home-gateway/enable-u60-home.sh --check

# 部署已构建的 agent 和网关规则
bash local-tools/u60-home-gateway/enable-u60-home.sh

# 恢复部署前的 agent、启动脚本和偏好，移除配套转发规则
bash local-tools/u60-home-gateway/enable-u60-home.sh --rollback
```

多台 ADB 设备同时连接时设置 `ANDROID_SERIAL`。首次备份保存在设备 `/data/u60-home-gateway-backup`，原有密码留在设备启动脚本中；工具不将其取回仓库。回滚保留 Tailscale 登录身份，依赖这份设备内备份。

## 为什么需要 agent 配合

上游默认使用 `userspace-networking`，启动或重新连接时设置 `--accept-routes=false`。单独执行一次 `tailscale set --accept-routes=true` 不足以让热点流量稳定经过 Tailscale。

启动环境变量 `ZTE_AGENT_TAILSCALE_GATEWAY=1` 让 fork 使用 `tailscale0` TUN、接受子网路由，并调用 `/data/local/tmp/u60-home-network.sh` 管理专用规则。`--netfilter-mode=off` 避免 Tailscale 改写固件管理的蜂窝防火墙。未设置这个变量时维持上游默认行为。

Tailscale Serve 将 tailnet TCP 9090 转到本机 9090，以保留原有认证和本机代理入口。覆盖安装上游发布的 agent 会失去 fork 的网关改动；应构建此 fork 的源码，并保留启动环境变量和网络脚本。

## 验证边界

旧版直启布局已验证：通过热点访问家庭网关的 ICMP / HTTP、转发及 NAT 计数、tailscaled 退出后由 agent 重启并恢复子网访问，以及规则重复应用和删除。整机重启、新 OTA 布局部署、另一个 tailnet 节点访问管理后台仍需单独实机验证。
