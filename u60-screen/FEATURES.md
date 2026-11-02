# 小屏功能清单

对应仓库基线 `03f86da` 及当前本地网关补丁。按 `zte-agent/src/server.rs` 的生产路由核对，共 198 个方法/路径组合；76 个页面入口，13 个分类。

此清单记录原型的界面设计和将来接入的接口。当前交互全部在浏览器内模拟，字段需要在实体接入阶段逐项对照请求/响应契约。

一级导航：概览、网络、应用、设备。页面中的“返回”回到上层；长内容向上滚动。

## Wi-Fi 与局域网

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [2.4 GHz 热点](preview/index.html#wifi2) | 保存热点设置 | `GET /api/wifi/status`<br>`PUT /api/wifi/settings` |
| [5 GHz 热点](preview/index.html#wifi5) | 保存热点设置 | `GET /api/wifi/status`<br>`PUT /api/wifi/settings` |
| [访客网络](preview/index.html#guest) | 保存访客设置 | `GET /api/wifi/guest`<br>`PUT /api/wifi/guest` |
| [连接设备](preview/index.html#clients) | 刷新设备 | `GET /api/network/clients`<br>`GET /api/network/client-details` |
| [LAN 与 DHCP](preview/index.html#lan) | 保存局域网设置 | `GET /api/router/lan`<br>`PUT /api/router/lan`<br>`GET /api/network/lan-status` |

## 移动网络

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [蜂窝数据](preview/index.html#mobile-data) | 保存连接设置、开启飞行模式、恢复蜂窝联网 | `GET /api/modem/status`<br>`GET /api/modem/data`<br>`PUT /api/modem/data`<br>`POST /api/modem/airplane`<br>`POST /api/modem/online` |
| [首选网络](preview/index.html#network-mode) | 应用网络模式 | `PUT /api/modem/network-mode` |
| [运营商选择](preview/index.html#operators) | 扫描运营商、注册所选网络 | `POST /api/modem/scan`<br>`GET /api/modem/scan/status`<br>`GET /api/modem/scan/results`<br>`POST /api/modem/register`<br>`GET /api/modem/register/result` |
| [APN 配置](preview/index.html#apn) | 列表新增/编辑/删除；添加 APN、使用自动配置 | `GET /api/router/apn/mode`<br>`PUT /api/router/apn/mode`<br>`GET /api/router/apn/profiles`<br>`PUT /api/router/apn/profiles`<br>`POST /api/router/apn/profiles`<br>`GET /api/router/apn/auto-profiles`<br>`POST /api/router/apn/profiles/delete`<br>`POST /api/router/apn/profiles/activate` |
| [流量与套餐](preview/index.html#usage) | 保存套餐设置 | `GET /api/data-usage`<br>`PUT /api/data-usage/limit`<br>`PUT /api/data-usage/cycle` |
| [无服务省电](preview/index.html#oos) | 保存策略 | `GET /api/modem/oos-policy`<br>`PUT /api/modem/oos-policy` |
| [断网自动恢复](preview/index.html#watchdog) | 保存恢复策略 | `GET /api/modem/conn-watchdog`<br>`PUT /api/modem/conn-watchdog` |

## 信号与锁频

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [实时信号](preview/index.html#signal-live) | 刷新信号 | `GET /api/network/signal` |
| [载波聚合](preview/index.html#carriers) | 详情/列表浏览 | `GET /api/network/signal` |
| [5G 频段偏好](preview/index.html#nr-bands) | 应用频段偏好、恢复自动选频 | `POST /api/cell/band/nr`<br>`POST /api/cell/band/reset` |
| [4G 频段偏好](preview/index.html#lte-bands) | 应用频段偏好、恢复自动选频 | `POST /api/cell/band/lte`<br>`POST /api/cell/band/reset` |
| [5G 小区锁定](preview/index.html#nr-lock) | 锁定小区、解除小区锁定 | `POST /api/cell/lock/nr`<br>`POST /api/cell/lock/reset` |
| [4G 小区锁定](preview/index.html#lte-lock) | 锁定小区、解除小区锁定 | `POST /api/cell/lock/lte`<br>`POST /api/cell/lock/reset` |
| [邻区扫描](preview/index.html#neighbors) | 重新扫描邻区 | `POST /api/cell/neighbors/scan`<br>`GET /api/cell/neighbors/nr`<br>`GET /api/cell/neighbors/lte` |
| [STC 小区策略](preview/index.html#stc) | 应用策略、重置 STC | `GET /api/cell/stc/params`<br>`PUT /api/cell/stc/params`<br>`GET /api/cell/stc/status`<br>`POST /api/cell/stc/enable`<br>`POST /api/cell/stc/disable`<br>`POST /api/cell/stc/reset` |
| [信号质量检测](preview/index.html#signal-test) | 开始检测、停止检测 | `POST /api/cell/signal-detect/start`<br>`POST /api/cell/signal-detect/stop`<br>`GET /api/cell/signal-detect/results`<br>`GET /api/cell/signal-detect/progress` |

## 路由与防火墙

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [网络接口](preview/index.html#wan) | 详情/列表浏览 | `GET /api/network/wan`<br>`GET /api/network/wan6`<br>`GET /api/network/rmnet`<br>`GET /api/network/lan-status` |
| [防火墙](preview/index.html#firewall) | 保存防火墙、查看过滤规则 | `GET /api/router/firewall`<br>`PUT /api/router/firewall/switch`<br>`PUT /api/router/firewall/level`<br>`GET /api/router/firewall/filter-rules` |
| [NAT 类型](preview/index.html#nat) | 应用 NAT 模式 | `GET /api/router/firewall`<br>`PUT /api/router/firewall/nat` |
| [DMZ 主机](preview/index.html#dmz) | 保存 DMZ | `GET /api/router/firewall`<br>`PUT /api/router/firewall/dmz` |
| [UPnP](preview/index.html#upnp) | 保存 UPnP | `GET /api/router/firewall/upnp`<br>`PUT /api/router/firewall/upnp` |
| [端口转发](preview/index.html#ports) | 列表新增/编辑/删除；添加转发规则 | `GET /api/router/firewall/port-forward`<br>`POST /api/router/firewall/port-forward`<br>`PUT /api/router/firewall/port-forward/switch` |
| [QoS 流量优先级](preview/index.html#qos) | 保存 QoS | `GET /api/router/qos`<br>`PUT /api/router/qos` |
| [VPN 与 ALG 穿透](preview/index.html#vpn) | 保存穿透设置 | `GET /api/router/vpn`<br>`PUT /api/router/vpn` |
| [TTL 与跳数](preview/index.html#ttl) | 应用跳数设置 | `GET /api/router/ttl`<br>`PUT /api/router/ttl` |
| [域名过滤](preview/index.html#domain) | 保存过滤列表 | `GET /api/router/domain-filter`<br>`PUT /api/router/domain-filter` |

## DNS 与隐私

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [DNS 解析](preview/index.html#dns-config) | 测试解析、应用 DNS | `GET /api/router/dns`<br>`PUT /api/router/dns`<br>`GET /api/dns/status`<br>`POST /api/dns/test`<br>`PUT /api/dns/apply` |
| [加密 DNS](preview/index.html#doh) | 保存加密 DNS | `GET /api/doh/status`<br>`PUT /api/doh/config`<br>`POST /api/doh/enable`<br>`POST /api/doh/disable` |
| [DNS 缓存](preview/index.html#dns-cache) | 刷新缓存列表、清空缓存 | `GET /api/dns/cache`<br>`POST /api/dns/cache/clear`<br>`GET /api/doh/cache`<br>`POST /api/doh/cache/clear` |

## Tailscale

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [家庭内网](preview/index.html#tailscale-home) | 连接状态、拓扑、家庭连通性检测 | `GET /api/tailscale/status` |
| [连接与服务](preview/index.html#tailscale-service) | 显示登录指引、启用 Tailscale、停止 Tailscale、退出登录 | `GET /api/tailscale/status`<br>`POST /api/tailscale/setup`<br>`POST /api/tailscale/login-url`<br>`POST /api/tailscale/enable`<br>`POST /api/tailscale/disable`<br>`POST /api/tailscale/logout` |
| [Tailscale 设置](preview/index.html#tailscale-config) | 保存 Tailscale 设置 | `PUT /api/tailscale/config` |
| [Tailscale 版本](preview/index.html#tailscale-version) | 安装指定版本 | `POST /api/tailscale/update` |

## 短信

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [收件箱](preview/index.html#sms-inbox) | 写短信 | `POST /api/sms/list`<br>`POST /api/sms/read`<br>`POST /api/sms/delete` |
| [发送短信](preview/index.html#sms-compose) | 发送短信 | `POST /api/sms/send` |
| [短信存储](preview/index.html#sms-storage) | 刷新容量 | `GET /api/sms/capacity` |
| [自动转发规则](preview/index.html#sms-forward) | 列表新增/编辑/删除；新增转发规则 | `GET /api/sms/forward/config`<br>`PUT /api/sms/forward/config`<br>`POST /api/sms/forward/rules`<br>`PUT /api/sms/forward/rules`<br>`DELETE /api/sms/forward/rules`<br>`PUT /api/sms/forward/rules/toggle`<br>`POST /api/sms/forward/test` |
| [转发记录](preview/index.html#sms-forward-log) | 重试暂存消息、清空转发记录 | `GET /api/sms/forward/log`<br>`POST /api/sms/forward/log/clear`<br>`POST /api/sms/forward/retry` |

## SIM 与 eSIM

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [SIM 卡信息](preview/index.html#sim-info) | 详情/列表浏览 | `GET /api/sim/info`<br>`GET /api/sim/imei`<br>`GET /api/sim/lock-trials` |
| [解锁 SIM PIN / PUK](preview/index.html#sim-verify) | 验证并解锁 | `POST /api/sim/pin/verify`<br>`GET /api/sim/lock-trials` |
| [SIM PIN 保护](preview/index.html#sim-pin) | 应用 PIN 保护 | `POST /api/sim/pin/mode` |
| [修改 SIM PIN](preview/index.html#sim-change-pin) | 更改 PIN | `POST /api/sim/pin/change` |
| [网络锁解锁](preview/index.html#sim-network-unlock) | 提交解锁码 | `POST /api/sim/unlock`<br>`GET /api/sim/lock-trials` |
| [eSIM 配置文件](preview/index.html#esim-profiles) | 列表新增/编辑/删除；添加 eSIM | `GET /api/esim/profiles`<br>`GET /api/esim/chip`<br>`POST /api/esim/enable`<br>`GET /api/esim/switch`<br>`POST /api/esim/nickname`<br>`POST /api/esim/delete` |
| [下载 eSIM](preview/index.html#esim-download) | 预览套餐、取消下载、清除已完成结果 | `POST /api/esim/download`<br>`GET /api/esim/download`<br>`DELETE /api/esim/download`<br>`POST /api/esim/download/confirm`<br>`POST /api/esim/download/cancel` |
| [eSIM 通知回执](preview/index.html#esim-receipts) | 发送待处理回执 | `GET /api/esim/notifications`<br>`POST /api/esim/notifications/process` |

## 通话与卡片服务

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [通话](preview/index.html#phone) | 拨号、接听、挂断、静音 / 恢复、发送按键音 | `POST /api/call/dial`<br>`POST /api/call/hangup`<br>`POST /api/call/answer`<br>`GET /api/call/status`<br>`POST /api/call/dtmf`<br>`POST /api/call/mute` |
| [USSD 服务](preview/index.html#ussd) | 发送服务代码、回复当前会话、结束会话 | `POST /api/ussd/send`<br>`POST /api/ussd/respond`<br>`POST /api/ussd/cancel` |
| [SIM 工具菜单](preview/index.html#stk) | 提交回复、返回上级菜单、结束卡片会话 | `GET /api/stk/menu`<br>`POST /api/stk/select`<br>`GET /api/stk/session`<br>`POST /api/stk/respond`<br>`POST /api/stk/cancel`<br>`POST /api/stk/back` |

## 诊断工具

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [CPU、内存与温度](preview/index.html#resources) | 刷新系统状态 | `GET /api/cpu`<br>`GET /api/memory`<br>`GET /api/device/thermal`<br>`GET /api/device/system` |
| [进程管理](preview/index.html#processes) | 刷新进程、清理非必要服务 | `GET /api/system/top`<br>`POST /api/system/kill-bloat` |
| [互联网测速](preview/index.html#speedtest) | 开始测速、停止测速 | `GET /api/speedtest/servers`<br>`POST /api/speedtest/start`<br>`GET /api/speedtest/progress`<br>`POST /api/speedtest/stop` |
| [局域网测速](preview/index.html#lan-test) | 查看测试说明 | `GET /api/lan/ping`<br>`GET /api/lan/download`<br>`POST /api/lan/upload` |
| [AT 终端](preview/index.html#at-terminal) | 执行指令、清空显示 | `POST /api/at/send`<br>`GET /api/at/port` |
| [流量监视](preview/index.html#traffic) | 刷新统计 | `GET /api/network/traffic`<br>`GET /api/network/speed`<br>`GET /api/network/speeds` |
| [连接记录](preview/index.html#history) | 详情/列表浏览 | `GET /api/events`<br>`GET /api/dashboard/events` |

## 自动化

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [计划任务](preview/index.html#scheduler) | 列表新增/编辑/删除；新增计划任务 | `GET /api/scheduler/jobs`<br>`POST /api/scheduler/jobs`<br>`PUT /api/scheduler/jobs`<br>`DELETE /api/scheduler/jobs`<br>`PUT /api/scheduler/jobs/toggle` |
| [iPhone 实时活动](preview/index.html#live-activity) | 查看绑定说明、取消当前绑定 | `GET /api/live-activity`<br>`PUT /api/live-activity`<br>`DELETE /api/live-activity` |

## 电池与电源

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [电池详情](preview/index.html#battery-details) | 刷新电池信息 | `GET /api/battery`<br>`GET /api/device/battery-info`<br>`GET /api/device/charger` |
| [充电策略](preview/index.html#charge) | 保存充电策略 | `GET /api/device/charge-control`<br>`PUT /api/device/charge-control` |
| [省电模式](preview/index.html#power-save) | 应用省电模式 | `POST /api/device/power-save`<br>`PUT /api/device/power-save` |
| [快速启动](preview/index.html#fast-boot) | 保存启动设置 | `GET /api/device/fast-boot`<br>`PUT /api/device/fast-boot` |
| [自动休眠](preview/index.html#auto-sleep) | 保存休眠设置 | `GET /api/device/auto-sleep`<br>`PUT /api/device/auto-sleep` |
| [USB 与反向供电](preview/index.html#usb) | 应用 USB 设置 | `GET /api/usb/status`<br>`PUT /api/usb/mode`<br>`PUT /api/usb/powerbank` |

## 系统设置

| 页面 | 交互 | 接口映射 |
| --- | --- | --- |
| [设备信息](preview/index.html#device-info) | 详情/列表浏览 | `GET /api/device` |
| [屏幕与外观](preview/index.html#screen-prefs) | 深浅主题、自选强调色、保存屏幕偏好 | 新增屏幕本地设置；主题与配色在浏览器保存 |
| [HTTPS 与证书](preview/index.html#https) | 开启 HTTPS、更新证书、重置 HTTPS | `GET /api/https`<br>`POST /api/https/on`<br>`POST /api/https/renew`<br>`POST /api/https/reset` |
| [后台连接](preview/index.html#account) | 模拟会话过期、恢复演示会话 | `POST /api/auth/login` |
| [重启设备](preview/index.html#reboot) | 重启设备 | `POST /api/device/reboot` |
| [恢复出厂设置](preview/index.html#factory-reset) | 恢复出厂设置 | `POST /api/device/factory-reset` |
| [配置备份与恢复](preview/index.html#backup) | 说明与操作指引 | 配套端操作指引，agent 暂无接口 |

## 接入边界

- `Tailscale 在线`、`家庭节点在线`、`子网路由已接受`、`家庭地址探测成功`应分别建模；当前首页可达状态为场景模拟。
- PIN/PUK、锁网、恢复出厂、eSIM 删除等动作目前只有交互预演。真实版本需使用后端读取的最新状态并处理剩余尝试次数、超时及中途断连。
- 手机端扫码/输入激活码、Tailscale 授权、证书安装和局域网测速等跨设备流程，保持与配套端协作。
- 备份/恢复只有指引；没有添加一个实际上不存在的 agent API。
- 屏幕亮度、主题配色和熄屏为新增屏幕功能；预览支持运行时换色，实体版本需要接入设备显示、配置存储与背光驱动。

本次对照没有发现未映射的生产路由；这不等于所有接口字段及硬件执行结果已经验收。
