# Tailscale 子网网关

让 U60 热点客户端通过 Tailscale 访问已批准的子网路由。目标可以是办公网络、实验室或其他远端网络；部署时必须显式指定网段，不预设使用地点。

工具从指定 LAN 接口读取实际 IPv4 网段，支持最多 8 条目标 IPv4 CIDR，并拒绝与 LAN 重叠的网段。热点客户端无需安装 Tailscale，普通互联网流量继续使用原有出口。远端子网路由器需要在线，路由已在 Tailscale 管理后台批准，访问策略允许 U60 访问目标。

## 入口与配置

| 文件 | 用途 |
| --- | --- |
| `manage-gateway.sh` | Linux 电脑上的部署入口，调用 `deploy.py` |
| `deploy.py` | 只读检查、设备内备份、部署、路由检查与失败回滚 |
| `tailscale-gateway.sh` | 设备上的定向转发、NAT、TCP MSS 规则 |
| `/data/local/tmp/tailscale-gateway.conf` | 部署时生成的接口与子网配置 |
| `zte-agent/src/tailscale/mod.rs` | agent 的 TUN 模式、路由接收与规则恢复 |

设备配置是数据文件，不作为 shell 脚本执行。示例：

```ini
LAN_IF=br-lan
LAN_NET=10.8.0.0/23
REMOTE_NETS=172.20.10.0/24 10.20.0.0/16
```

规则仅匹配配置中的源、目标与接口，并用 `u60-tailscale-gateway` 注释标识归属。重复应用不增加重复规则，删除只移除这些规则；兼容迁移会清理同一规则组合上的旧标签，不清空防火墙。

## 构建与部署

当前部署器支持**旧版直启布局**：`/data/zte-agent` 为普通文件，`/data/local/tmp/start_zte_agent.sh` 直接启动 agent。遇到新版 OTA 槽位或 supervisor 布局时，在任何部署或回滚写入前拒绝执行。

先准备交叉编译工具链（见根目录 `CONTRIBUTING.md`），在仓库根目录运行：

```bash
cargo build --locked --release --target aarch64-unknown-linux-musl -p zte-agent
cp target/aarch64-unknown-linux-musl/release/zte-agent local-tools/tailscale-gateway/zte-agent
(cd local-tools/tailscale-gateway && sha256sum zte-agent > zte-agent.sha256)
```

以下网段为命令示例，执行时替换为已批准的实际子网：

```bash
# 只读检查；首次部署要求当前为 userspace 模式
bash local-tools/tailscale-gateway/manage-gateway.sh --check --remote-cidr 10.20.0.0/16

# 多个子网可重复传参；LAN 默认从 br-lan 识别
bash local-tools/tailscale-gateway/manage-gateway.sh \
  --remote-cidr 10.20.0.0/16 --remote-cidr 172.20.10.0/24

# 回滚使用首次备份，不需要重新指定网段
bash local-tools/tailscale-gateway/manage-gateway.sh --rollback
```

`--lan-interface` 可指定其他热点 LAN 接口；多台 ADB 设备同时连接时使用 `ANDROID_SERIAL`。构建产物和校验文件被 Git 忽略。

路由与转发规则就绪不等于每个目标服务可访问。部署器不再假定子网的 `.1` 主机必然提供 ICMP 或 HTTP；实际服务由连通性检测或热点客户端验证。

## 兼容与回滚

原 `local-tools/u60-home-gateway/` 目录保留兼容入口；旧命令仍会进入新实现，但新部署必须提供 `--remote-cidr`。旧目录中的本地构建产物仍可通过兼容入口使用。

agent 优先调用 `/data/local/tmp/tailscale-gateway.sh`；新脚本不存在时继续使用已安装的 `/data/local/tmp/u60-home-network.sh`。更新仓库和编译不会改写设备上的脚本、路由或防火墙。部署新脚本时同时安装旧文件名的转发入口。

新备份位于 `/data/tailscale-gateway-backup`。检测到旧 `/data/u60-home-gateway-backup` 时复用原备份，保持首次备份的来源和回滚能力；密码仍留在设备启动文件内，不取回仓库。回滚保留 Tailscale 登录身份。

已经处于网关模式的设备无需因命名变更重新部署。部署器会拒绝直接覆盖运行中的 TUN 配置；迁移前先按旧备份回滚，再显式选择子网部署。修改已安装的网关配置前，应先用旧配置执行 `remove`，再修改配置并 `apply`，避免留下旧网段规则。

## agent 行为与验证范围

`ZTE_AGENT_TAILSCALE_GATEWAY=1` 启用 `tailscale0`、接收子网路由，并在启动或恢复连接时应用专用规则。`--netfilter-mode=off` 保留固件的蜂窝防火墙。未开启该选项时维持上游默认行为。Tailscale Serve 将 tailnet TCP 20601 代理到本机 20600，沿用后台认证。

旧版固定网段工具曾在 U60 Pro B31、Tailscale 1.104.4 上验证热点客户端访问、转发计数和 tailscaled 重启恢复。新版配置化规则已通过无设备参与的测试：多个 CIDR、非 `/24` LAN、重复应用、定向删除、旧标签迁移、非法或重叠网段拒绝；部署器仍检查 Tailscale 1.104.4。新版部署和整机重启恢复尚需单独实机验证。
