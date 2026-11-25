# Velo v0.13.0 界面截图

[项目首页](../../README.md) · [用户指南](../user-guide.md) · [宣传片素材](video-v0.13/README.md) · [采集清单与校验值](capture-manifest.json)

2026-11-26（北京时间）采集了 20 张 Windows 实机截图。使用 v0.13.0 标签源码的本机 Software 正式产品构建，Slint 1.18.1、winit。系统为 Windows 11 x64 26340，桌面 1920 × 1080，缩放 100%。 [版本说明](../releases/0.13.0.md)

## 启动器与工作台

- 首页展示固定项目、常用项目和系统快捷。「专注工作」组合已固定。 [浅色首页](home-light.png) · [深色首页](home-dark.png)
- 搜索「演示」查看同一组 6 个应用的名称、图标和列表路径。 [列表](search-list.png) · [六列网格](search-grid.png) · [7-Zip 原生菜单](context-menu.png)
- 搜索「会议纪要」返回 5 个示例文件与文件夹，截图选中了文本纪要。 [文件搜索](everything-search.png) · [Everything 设置](everything-settings.png)
- 「专注工作」组合配置了记事本、计算器和字符映射表。 [启动组合编辑器](launch-groups.png)
- 短按侧键打开上方全局动作和下方应用动作。长按划向上方后，快捷盘提示「松开执行」。 [工作台](workspace-panel.png) · [快捷盘](workspace-gesture.png)
- 设置和自定义项目页面。 [外观](appearance.png) · [自定义项目](custom-apps.png) · [配置备份](config-backup.png) · [关于](about.png)

## 官网用图

独立官网的主题切换使用 `/assets/home-light.webp` 和 `/assets/home-dark.webp`。对应文件是 640 × 660 的无损 WebP，解码像素与上面的 PNG 一致。 [浅色 WebP](site/home-light.webp) · [深色 WebP](site/home-dark.webp)

## 采集环境

Velo 使用独立实例、数据目录与热键。固定项、组合和使用记录使用演示数据，应用名称与图标来自本机真实应用。Everything 使用独立的 1.4.1.1015 客户端配置，只索引 `D:\VeloDemo` 的公开样例。Everything 和工作台默认关闭。

启动器以原始窗口位图采集。原生菜单和记事本按窗口可见边界截取，工作台与快捷盘使用桌面验证脚本的合成侧键和隔离编辑器。截图保留原比例，鼠标指针与操作光晕在画面外。清单记录尺寸、采集时间、来源、构建提交与 SHA-256。 [采集清单](capture-manifest.json)

旧版 Go/Wails 图片随历史源码保存。 [v0.9.11 截图](https://github.com/HBLADEH/velo-launcher/tree/v0.9.11/docs/images) · [Rust 验收基线](../baselines/README.md)
