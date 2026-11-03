# U60 Sans

`U60Sans.ttf` 是 Noto Sans SC 的完整字库，保留可变字重轴（100–900），统一更名为 U60 Sans。默认正文使用 500，重点使用 600–650。保留真实字重可以避免把所有文字绘制成同一份 Regular 轮廓；小屏辅助文字至少 12 px，多数中文辅助文字为 13 px。

- 上游：[google/fonts, ofl/notosanssc](https://github.com/google/fonts/tree/a85815a42757630ce188fdad368c2dfc444d4773/ofl/notosanssc)
- 源文件：`NotoSansSC[wght].ttf`
- 源 SHA-256：`a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da`
- 许可：[SIL Open Font License 1.1](OFL.txt)。版权声明保留在字体中。
- 工具：fonttools 4.60.1；没有使用或分发原厂字体。

普通构建直接使用仓库内的完整字体，不需要 Python 字体工具或联网下载。需要重新生成时，使用上述固定版本的源字体（脚本验证 SHA-256）：

```bash
python3 u60-screen/scripts/subset-font.py /path/to/NotoSansSC\[wght\].ttf
```

默认保留上游全部字符，支持真实中文短信、SSID 和设备名称。它仍然不是覆盖所有语言与彩色 emoji 的通用字库。`--ui-only` 可生成仅包含 `ui/*.slint`、`src/**/*.rs`、`assets/*.json` 和 ASCII 的预览子集；该模式不能用于正式机身构建。完整字库约 18 MB，增加静态产物体积以换取动态内容的文字覆盖。

Slint 软件渲染默认不启用 hinting，并按 1/4 像素放置水平字形。本程序的 [可选渲染补丁](../vendor/README.md) 启用 hinting，并将字形起点取整到像素；颜色与 RGB565 帧缓冲保持正常输出，不对画面整体锐化。

## 韩文字形

`U60Hangul.ttf` 补充完整现代韩文音节、Jamo、ASCII 与常用标点，保留可变字重。韩语界面优先使用此字体，其他文字由已嵌入的 U60 Sans 回退显示。约 4.5 MB；正式构建不依赖系统字体。

- 源：[google/fonts, ofl/notosanskr](https://github.com/google/fonts/tree/a85815a42757630ce188fdad368c2dfc444d4773/ofl/notosanskr)
- 源文件：`NotoSansKR[wght].ttf`
- 源 SHA-256：`194018e6b2b293a7964f037b25c0249ce1418bc9ab3c971060a03aa57861e252`
- 同样遵循 [OFL 1.1](OFL.txt)，版权保留在字体中，修改后更名为 U60 Hangul。

```bash
python3 u60-screen/scripts/hangul-font.py /path/to/NotoSansKR\[wght\].ttf
```
