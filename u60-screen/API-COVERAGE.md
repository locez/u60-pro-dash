# 原生屏幕接口覆盖

核对日期：2026-11-03。接口统计基线：`9ce5bac`，加上 `ef70b18` 的 10 个代理接口。Wi-Fi 分享复用已有读取接口。

上述基线后台有 **218 个 HTTP 方法与路径组合**，原生屏幕直接接入 **199 个，约 91.3%**。若只按路径去重，则为 170 / 187；同一路径的 GET、POST、PUT、DELETE 是不同操作，本文采用前一种口径。

## 服务分类调整后的增量

代理接口已按服务隔离，旧 10 个接口保留兼容别名；新路径 /api/proxy/{service}/... 对应原有 10 项操作，另增服务列表和保存配置读取。按实际可访问方法/路径计数，增加 12 项，总计 230 项；原生的 7 项代理调用改到对应服务路径，因此直接调用数量仍为 199 项（约 86.5%）。新增别名并非缺失功能，下表保留原统计基线以供对照。服务列表和配置读取由 Web 提供，其他服务尚未实现。

## 统计口径

- 后台来源：[server.rs](../zte-agent/src/server.rs) 的 `precheck()` 与 `route_inner()`，包含登录、SSE 和局域网测速，排除测试、重复路由声明及网页 `/ui/` 路由。
- 原生来源：[features/](src/features/) 的读取契约、操作计划，以及 [agent/http.rs](src/agent/http.rs) 的登录与 SSE。动态路径按代码允许的操作逐项展开，多个页面共用同一接口只计一次。
- “接入”表示已有界面流程与真实请求代码，不表示所有接口字段、固件版本或硬件执行结果均已验收。受设备能力限制而禁用的操作仍属于代码已接入。
- 计划任务的自定义请求不算对任意后台接口的直接接入；浏览器原型中的接口映射和操作指引也不算实际调用。
- 当前 82 个设计入口包含本地流程与配套操作指引，不能直接用作接口覆盖率。此前 102 次设备读取包含跨页面重复调用，也不是唯一接口数。

| HTTP 方法 | 后台 | 原生直接接入 | 未直接调用 |
| --- | ---: | ---: | ---: |
| GET | 85 | 76 | 9 |
| POST | 88 | 81 | 7 |
| PUT | 41 | 38 | 3 |
| DELETE | 4 | 4 | 0 |
| 合计 | 218 | 199 | 19 |

原功能清单的 198 个接口对应早期基线；当前增加了 4 个 WireGuard 和 6 个后台在线更新接口，以及 10 个代理服务接口。

## 未直接调用的 19 个接口

前 12 个使用其他接口实现相应能力，仍不计入直接接入数量。

| 接口 | 当前原生实现 |
| --- | --- |
| `GET /api/router/dns` | DNS 页面使用 `GET /api/dns/status` |
| `PUT /api/router/dns` | DNS 页面使用统一的 `PUT /api/dns/apply` |
| `GET /api/doh/status` | 加密 DNS 页面使用 `GET /api/dns/status` |
| `PUT /api/doh/config` | 使用统一的 `PUT /api/dns/apply` |
| `POST /api/doh/enable` | 通过统一 DNS 配置切换模式 |
| `POST /api/doh/disable` | 通过统一 DNS 配置切换模式 |
| `GET /api/doh/cache` | 缓存页面使用 `GET /api/dns/cache` |
| `POST /api/doh/cache/clear` | 使用 `POST /api/dns/cache/clear` |
| `GET /api/network/clients` | 客户端页面使用 `GET /api/network/client-details`，不把地址提示记录当作在线设备 |
| `GET /api/network/speed` | 流量页面使用 `GET /api/network/speeds`，首页另有实时推送 |
| `GET /api/stk/session` | 会话变化来自 `GET /api/events` 的 `stk` 主题 |
| `GET /api/dashboard/events` | 这是网页片段流；原生使用 JSON SSE `GET /api/events` |

另 4 个依赖配套设备，原生只有相关说明或状态管理。

| 接口 | 尚需配套端完成的部分 |
| --- | --- |
| `GET /api/lan/ping` | 热点客户端到 U60 的时延测试 |
| `GET /api/lan/download` | 热点客户端的下载吞吐测试 |
| `POST /api/lan/upload` | 热点客户端的上传吞吐测试 |
| `PUT /api/live-activity` | iPhone 创建并绑定实时活动；屏幕可读取状态和解除绑定 |

配置备份/恢复只有指引，当前后台没有对应 API，因此不在 218 个接口的分母内。

新增的 3 个代理维护接口由 Web 管理页提供，原生有相关操作指引：

| 接口 | Web 功能 |
| --- | --- |
| `POST /api/proxy/config` | 上传或粘贴配置 |
| `POST /api/proxy/subscription` | 从 HTTPS URL 导入配置 |
| `POST /api/proxy/install` | 安装与更新内核 |

## 实机可用性

此次统计只核对代码，没有执行新的设备操作。之前的 B31 检查表明：

- UPnP、QoS、STC 缺少固件 ubus 方法；域名过滤返回空对象；VPN 穿透仅能读取 SIP ALG。相应不可用操作已禁用。
- 首次常驻部署已安装包含代理、WireGuard 和成对更新限制的后台；下列历史统计不代表新的硬件写入已经验收。
- 短信发送、通话、网络切换、eSIM 等写入仍未逐项实机验收；eSIM/STK 也未纳入无人值守读取检查。

具体测试与设备读取结果见 [LIVE.md](LIVE.md)。
