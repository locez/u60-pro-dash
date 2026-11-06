# U60 产品界面设计稿

已选方向：**Web 采用 A「克制专业」，设备屏幕采用 C「紧凑高密度」**。本目录是 React + Tailwind 交互原型，用于评审页面、导航与关键流程。

当前包含 8 个 Web 主分类、38 个页面或子页视图，以及可操作的 320 × 480 屏幕原型。现有屏幕目录的 82 个功能入口均已记录新归属；入口映射不等于接口实现完成。所有数据都是虚构示例，操作只改变预览状态。

## 打开预览

在仓库根目录运行：

```bash
bash design/product-ui/preview.sh
```

打开 **http://127.0.0.1:5176/**。顶部可切换 Web、设备屏幕、功能地图及 Web 浅色／深色。场景菜单可查看蜂窝断网和可管理 eUICC 卡的展示差异。

| 入口         | 地址                                          |
| ------------ | --------------------------------------------- |
| Web 概览     | http://127.0.0.1:5176/?view=web               |
| 组网与服务   | http://127.0.0.1:5176/?view=web&page=services |
| 设备屏幕     | http://127.0.0.1:5176/?view=screen            |
| 功能地图     | http://127.0.0.1:5176/?view=map               |
| 初版三种风格 | http://127.0.0.1:5176/?explore=1              |

## 本轮可评审的内容

- Web：概览、Wi-Fi、连接设备、移动网络、短信、组网与服务、局域网、系统设置。每个服务独立管理自己的配置、状态与维护。
- 表单：开关与选项即时应用，文字输入在回车或结束输入时应用；只对中断连接的修改提示确认，取消保留原值。Wi-Fi 名称修改会同步到分享卡片。不同频段名称提供独立二维码。
- 服务：独立启停、路由与实际可达性分开、日志筛选、安装更新入口、长配置完成编辑后自动校验、保存与 reload 的流程。
- 屏幕：状态／连接／管理三个主入口，服务详情、供电方式、连接码、长短信与进程表；长按 1 秒解锁和可选 PIN 流程。
- 功能地图：查询已有功能迁往哪个 Web 页面和哪个屏幕分类。

屏幕预览旁边有快捷场景按钮。PIN 演示值为 `1234`；语言设置展示选项与保存流程，正文当前以简体中文评审。

屏幕强调色可在预览旁的色板、屏幕「管理 → 屏幕与安全 → 强调色」，或 Web 的屏幕设置中调整。提供六种预设和 `#RRGGBB` 自定义色，浏览器会记住选择。重点读数、趋势线、选中导航、按钮和解锁提示使用同一强调色；文字自动保证对比度，充电与告警保留各自的状态颜色。[橙色示例](previews/product-screen-accent-orange.png)。

屏幕支持浅色、深色和跟随系统，主题与强调色独立保存，覆盖详情、弹层、锁屏和 PIN 页面。可在预览旁、屏幕「管理 → 屏幕与安全 → 界面外观」或 Web 屏幕设置中切换。

「首页布局」可配置是否显示信号详情、Wi-Fi 连接卡片和组网服务摘要，也可单独隐藏某项服务。三个快捷指标支持在线设备、已用／剩余流量、CPU、内存、温度、电量、运行时间或不显示；选择已有指标会交换位置。设置在预览中即时生效并保留到下次打开；隐藏摘要只影响首页展示，不停止服务。

屏幕服务配置页的运行模式选完即生效；流量范围仅在服务运行且需要停止后重启时提示确认，停止状态下直接保存。两端使用相同规则，不再要求额外点击“应用已保存配置”。此处仍为示例交互，真实设备的保存、重载、重启与回读验证在后续接口接入时完成。

公开截图使用中性示例名称的发布预览，例如 `http://127.0.0.1:5176/?view=screen&presentation=public`；切换页面与初版风格会保留该模式。

实际设备继续使用 Rust + Slint。浏览器里的屏幕原型用于审阅尺寸、信息密度和交互；它不替代设备版的字体、触摸、性能与功耗验证。

## 截图

| 页面         | 截图                                                                                                                                             |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Web 首页     | [浅色](previews/product-overview.png) · [深色](previews/product-overview-dark.png) · [手机宽度](previews/product-mobile.png)                     |
| Wi-Fi        | [主网络与连接码](previews/product-wifi.png)                                                                                                      |
| 移动网络     | [信号与频段](previews/product-internet.png)                                                                                                      |
| 连接设备     | [设备表](previews/product-devices.png)                                                                                                           |
| 组网与服务   | [服务入口](previews/product-services.png) · [Tailscale 详情](previews/product-tailnet.png)                                                       |
| 短信         | [收件箱与正文](previews/product-messages.png)                                                                                                    |
| 局域网       | [地址与 DHCP](previews/product-lan.png)                                                                                                          |
| 系统设置     | [电源与电池](previews/product-system.png)                                                                                                        |
| 设备屏幕     | [首页](previews/product-screen-home.png) · [连接列表](previews/product-screen-connections.png) · [服务详情](previews/product-screen-tailnet.png) |
| 屏幕关键流程 | [锁屏](previews/product-screen-lock.png) · [双频连接码](previews/product-screen-wifi.png) · [长短信底部](previews/product-screen-sms.png)        |

[浅色首页](previews/product-screen-home-light.png) · [浅色锁屏](previews/product-screen-lock-light.png) · [自定义指标与隐藏服务](previews/product-screen-home-custom.png) · [首页布局设置](previews/product-screen-layout.png) · [可编辑的运行设置](previews/product-screen-runtime.png)

[页面层级与实现约束](DESIGN.md) · [82 项功能归属数据](src/features.json)

## 构建与验证

```bash
cd design/product-ui
npm ci --ignore-scripts
npm run build
```

构建产物在本目录 `dist/`。资源与字体随包提供，无 CDN 依赖。预览服务器仅监听本机；构建不接入现有设备部署流程。

已通过 TypeScript 与 Vite 构建、Chromium 交互检查：

- 38 个页面／子页在 1440 和 390 像素宽度下无整页横向溢出；长表格在自身区域内横向滚动。
- 独立服务开关、普通选项即时应用、文字完成输入、无效输入保留、断连确认与取消、双频分享、长配置完成后自动保存与重载可操作。
- 屏幕保持 320 × 480，首页内容完整放在导航上方；两个二维码均为 106 × 106，长名称不会压缩二维码。
- 长短信可以滚动至末尾；短按不解锁，长按 1 秒可解锁，可选 PIN 流程可用。
- 所有页面请求均留在本机，没有浏览器运行错误。

这是设计阶段的验证。设备 API、实际配置校验、安装升级、完整多语言、原生屏幕迁移与实机回归属于后续实现工作；少用的高级功能目前先确定入口，详见设计说明中的覆盖边界。
