# Velo v0.13 宣传片截图素材

按 45 秒横版分镜列出原尺寸 PNG、旧版对照和透明 Logo。Velo 界面保留窗口比例，字幕和键帽可在 1920 × 1080 时间线叠加。 [采集清单](../capture-manifest.json)

- **00–07 秒 · 启动应用**　呼出后搜索「记事本」，按 `Enter` 打开空白的 Windows 记事本。 [浅色首页](../home-light.png) · [记事本搜索](notepad-search.png) · [已打开的记事本](notepad-open.png)
- **07–10 秒、41–45 秒 · 标题与片尾**　使用 1254 × 1254 的透明渐变 Logo。 [原版 Logo](velo-logo.png)
- **10–18 秒 · 原生重构**　以搜索框对齐两版窗口。内存口径为重构期同机 `hidden-5m` 私有工作集，含 WebView2 子进程的 Go/Wails v0.9.11 进程树为 **120.14 MiB**，Rust + Slint Software 业务测量构建为 **19.84 MiB**。 [v0.9.11 浅色首页](home-go-v0.9.11.png) · [v0.13.0 浅色首页](../home-light.png) · [新版列表](../search-list.png) · [测量记录](../../baselines/2026-11-04-slint-rs12/README.md)
- **18–27 秒 · 启动组合**　本批用「专注工作」演示记事本、计算器和字符映射表。一次 `Enter` 后，Velo 显示已发出 3 个启动请求。 [组合编辑器](../launch-groups.png) · [组合搜索](launch-group-search.png) · [启动回执](launch-group-result.png)
- **27–36 秒 · Everything**　按 `Tab` 进入文件模式，搜索「会议纪要」，用 `↓` 选中文本纪要，再按 `Enter` 打开。 [文件模式](file-mode.png) · [选中示例文件](../everything-search.png) · [已打开的纪要](meeting-notepad.png) · [演示用文本](Velo示例-会议纪要.txt)
- **36–41 秒 · 工作台与手势测试版**　短按打开双区矩阵，长按划向上方后高亮启动器，并显示「松开执行」。 [工作台](../workspace-panel.png) · [快捷盘](../workspace-gesture.png)
