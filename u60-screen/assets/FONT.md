# U60 Sans

`U60Sans.ttf` 是 Noto Sans SC 的界面字形子集，保留可变字重轴（100–900），统一更名为 U60 Sans。默认正文使用 500，重点使用 600–650。保留真实字重可以避免把所有文字绘制成同一份 Regular 轮廓；小屏辅助文字至少 12 px，多数中文辅助文字为 13 px。

- 上游：[google/fonts, ofl/notosanssc](https://github.com/google/fonts/tree/a85815a42757630ce188fdad368c2dfc444d4773/ofl/notosanssc)
- 源文件：`NotoSansSC[wght].ttf`
- 源 SHA-256：`a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da`
- 许可：[SIL Open Font License 1.1](OFL.txt)。版权声明保留在字体中。
- 工具：fonttools 4.60.1；没有使用或分发原厂字体。

普通构建直接使用仓库内的字体，不需要 Python 字体工具或联网下载。添加新中文文案后，用上述固定版本的源字体重新生成：

```bash
python3 u60-screen/scripts/subset-font.py /path/to/NotoSansSC\[wght\].ttf
```

脚本收集 `ui/*.slint`、`src/**/*.rs`、`assets/*.json` 的字形，并包含基础 ASCII。当前字体服务于固定 UI 和演示内容；接入真实短信、任意设备名称时需要额外提供完整 CJK 字库或受控字体回退，不能假定子集覆盖用户输入。

Slint 软件渲染默认不启用 hinting，并按 1/4 像素放置水平字形。本程序的 [可选渲染补丁](../vendor/README.md) 启用 hinting，并将字形起点取整到像素；颜色与 RGB565 帧缓冲保持正常输出，不对画面整体锐化。
