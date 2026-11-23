# 界面资源

- `logo.png`：Velo 的 128 × 128 标志，用于窗口顶部和项目 README。解码为 RGBA 时为 65,536 字节；这是静态图像大小，不是运行内存测量。
- `velo.ico`：Windows 程序、安装器与快捷方式使用的多尺寸图标。
- `fluent/`：来自 [Microsoft Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons) 的本地 SVG 子集，采用 MIT 许可，完整文本见 [fluent/LICENSE.txt](fluent/LICENSE.txt)。用于界面控件与系统快捷的语义图标。

这些固定资源由 Slint 编译器嵌入程序，运行时无需网络获取。应用与系统工具的原生图标由 Windows 平台层在后台按可见范围提取，经磁盘和有界内存缓存加载。缺少原生图标时，系统快捷使用 Fluent 语义图标，应用使用名称首字符。

新增或替换资源时保留来源和许可，优先复用已有图标；动画预生成入口为 [generate-rust-rotating-icons.py](../../scripts/generate-rust-rotating-icons.py)。
