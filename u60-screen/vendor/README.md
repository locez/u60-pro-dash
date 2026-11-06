# Slint 软件字体补丁

`i-slint-renderer-software/` 来自 crates.io 的 **1.18.1** 发布包，许可证见源文件 SPDX 和本目录上层的 `licenses/GPL-3.0-only.txt`。

只修改两个文件：

1. `Cargo.toml` 增加默认关闭的 `u60-pixel-hinting` feature。
2. `fonts/vectorfont.rs` 在开启该 feature 时，将每像素 4 个水平位置改成 1 个，并向 Swash 传入 `.hint(true)`。

主 crate 显式启用此 feature。U60 的 320×480/RGB565 小屏上，整像素字形起点和 hinting 优先保证小字清晰度。代价是字间距可能出现整数像素取整；默认关闭该 feature 时保持上游的平滑字间距。这里没有对整幅画面做锐化，也没有修改 DRM、旋转或颜色转换。

升级 Slint 时，从对应版本发布包重新导入，重新核对这两处修改，跑原生页面/旋转测试，并在物理屏幕比较小字。不要用本地 Cargo 缓存里的未记录修改代替此补丁。字体字形和许可另见 `assets/FONT.md`。
