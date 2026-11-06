# 开源组件与参考资料

本目录自编写源码使用仓库的 MIT 许可。原生程序依赖 Slint 1.18.1；Slint 的可选许可为 GPL-3.0-only、Slint Royalty-free 2.0 或商业许可。本项目的开源嵌入式程序按 **GPLv3 路径**使用 Slint，分发链接后的可执行文件时必须遵守 GPLv3，提供对应源码、依赖版本和构建方法。源码文件保留 MIT 标识不代表组合后的可执行文件可以只按 MIT 分发。

- [Slint 1.18.1 许可说明](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSE.md)
- [GPL-3.0-only 完整文本](licenses/GPL-3.0-only.txt)
- [固定依赖版本](Cargo.lock) · [构建方法](NATIVE.md)

Slint 的 Royalty-free 许可不覆盖本设备的嵌入式用途。闭源嵌入式分发需要另行核对商业许可。

为改善 RGB565 小屏的文字清晰度，固定版本的软件渲染 crate 随源码放在 `vendor/`。仅增加可选的字体 hinting 与整像素起点，见 [补丁说明](vendor/README.md)。该第三方代码保留上游版权和许可，不属于本项目自编写的 MIT 源码。

Noto Sans SC 的字形子集遵循 SIL OFL 1.1，见 [字体来源与生成方式](assets/FONT.md) 和 [OFL 文本](assets/OFL.txt)。字体嵌入可执行文件，设备不需要原厂字体。

硬件适配参考 `U60PRO-screen-remix-skill-20261102.zip` 中公开的 Linux DRM、evdev 和背光用法，并通过当前 MU5250/B31 实机核对。Rust 实现使用 `drm`、`evdev` 和 Slint 公共 API；未复制或分发 ZIP 中的固件、原厂程序、字体、设备备份或安装脚本。参考包不是本项目的安装依赖。
