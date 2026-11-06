# U60 首页风格设计稿

React + Tailwind 的三种首页方向。当前只评审 Web 与 320 × 480 设备屏幕首页；使用同一组虚构数据，不接入路由器 API。风格选定后再展开完整页面与导航层级。

| 方向 | Web 首页 | 屏幕首页 |
| --- | --- | --- |
| A · 克制专业 | 连接拓扑、速率趋势、设备列表、独立服务状态 | 连接状态、上下行、连接码与服务状态 |
| B · 圆润轻盈 | 设备主视觉、常用连接操作、设备图标与套餐 | 大状态、大触点、直接扫码连接 |
| C · 紧凑高密度 | 指标条、蜂窝链路、设备表、服务与事件 | 速率、信号、流量、温度与连接码 |

## 打开预览

```bash
bash design/product-ui/preview.sh
```

浏览器打开 `http://127.0.0.1:5176/`，顶部切换 A / B / C、Web / 设备屏幕，以及浅色 / 深色。

也可直接访问：

- A：`http://127.0.0.1:5176/?style=a`
- B：`http://127.0.0.1:5176/?style=b`
- C：`http://127.0.0.1:5176/?style=c`
- 屏幕并排比较：`http://127.0.0.1:5176/?view=screen`

分享 Wi-Fi 弹窗、密码显隐、图表时间范围、服务独立开关可交互。页面导航仅用于观察首页的视觉布局。所有开关只改变内存中的示例状态；二维码、地址和密码都是演示值。屏幕预览使用浏览器原始像素尺寸，实际设备的字体渲染和功耗需在后续原生实现中验证。

## 截图与构建

[三种首页并排对比](previews/compare.png) · [屏幕首页](previews/screens.png)

| 方向 | 桌面 | 手机 | 深色 |
| --- | --- | --- | --- |
| A | [预览](previews/web-a.png) | [预览](previews/mobile-a.png) | [预览](previews/web-a-dark.png) |
| B | [预览](previews/web-b.png) | [预览](previews/mobile-b.png) | [预览](previews/web-b-dark.png) |
| C | [预览](previews/web-c.png) | [预览](previews/mobile-c.png) | [预览](previews/web-c-dark.png) |

```bash
cd design/product-ui
npm ci --ignore-scripts
npm run build
```

构建产物位于本目录的 `dist/`，可用任意静态文件服务预览。字体随包提供，无需 CDN；图表使用 SVG，不引入常驻动画或定时轮询。

本地浏览器验证覆盖：三版在 390、820、1440 像素宽度下无横向溢出；三个屏幕均为 320 × 480、内容不遮挡底部导航；分享弹窗、密码显隐、图表切换与服务独立启停可用；页面没有外部请求。
