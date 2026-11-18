# Rust + Slint 界面截图

[项目首页](../../README.md) · [用户指南](../user-guide.md) · [采集清单与校验值](capture-manifest.json)

本组 **12 张真实 Windows 窗口截图**于 2026-11-18 更新，展示 0.11.0 Software 开发构建的简体中文界面。中英文 README 共用这些图片。0.11.0 尚未发布，Everything 与启动组合不属于已发布的 v0.10.0。

## 构建与环境

- Velo 0.11.0、Rust 1.99.1、Slint 1.18.1、winit、Software renderer，正式 `product` feature，使用独立实例和演示数据目录运行。
- Windows 11 专业版 Insider Preview，10.0.26340；原生 DPI 96、缩放 100%，窗口宽度为 640 像素。
- 构建前完成 `./scripts/verify-rust.ps1 -Product -SmokeTest`；Software / FemtoVG 各 117 项 Rust 测试及完整原生、窗口、输入和页面检查通过。
- 截图 exe SHA-256：`13A52AEDB2ABF3DECC5E1FA223979F2E66B041EC198556F6803BF6050D8C6F73`。完整构建身份与验证范围见[本轮记录](../baselines/2026-11-18-project-review/README.md)。

构建仍来自未提交工作区；源码指纹记录实际输入，不把当前 Git 提交当成干净发行构建。截图用于说明界面与操作，不构成新的性能、跨机器或真实 IME 验收。

## 场景与用途

- [home-light.png](home-light.png)、[home-dark.png](home-dark.png)：640 × 660，同一组首页的深浅主题。固定、常用与系统快捷展示于同一窗口，更多内容可滚动查看。
- [search-list.png](search-list.png)：640 × 480，查询“演示”，展示六项公开应用及真实安装路径。
- [search-grid.png](search-grid.png)：640 × 316，同一组结果的六列网格；保留产品实际动态高度。
- [context-menu.png](context-menu.png)：640 × 480，7-Zip 的 Windows 原生右键菜单，含打开、安装目录、管理员启动和取消置顶。
- [appearance.png](appearance.png)：640 × 660，主题预览、浅色主题及网格排列选项。
- [everything-search.png](everything-search.png)：640 × 504，真实 Query2 IPC 返回的六项隔离示例结果，包含文件夹、TXT、Markdown 和 CSV。
- [everything-settings.png](everything-settings.png)：640 × 660，文件搜索开关与可选程序路径。
- [launch-groups.png](launch-groups.png)：640 × 660，“专注工作”组合编辑器，展示筛选后的应用库和两个成员；拍摄期间没有执行组合启动。
- [custom-apps.png](custom-apps.png)：640 × 660，拖入、选择文件、粘贴路径与公开应用列表。
- [config-backup.png](config-backup.png)：640 × 660，备份入口及恢复说明；此图展示入口，不表示正在执行或已经完成备份。
- [about.png](about.png)：640 × 660，当前版本、更新开关和许可入口。

## 数据与采集方式

固定项、自定义应用、别名和使用记录为文档准备的演示数据；自动索引与应用图标来自本机 Windows 原生发现。应用数量、常用排序与所选偏好属于演示环境，不是新安装默认值。截图中启用了 Everything，产品默认关闭。

文件搜索使用已有 Everything 1.4.1.1032 程序、独立 INI 与仅包含六项公开内容的文件夹索引。图片没有包含私人文件或用户名。拍摄使用独立热键，未使用正式 `%LOCALAPPDATA%\Velo` 数据。

图片直接由窗口原始位图保存为 PNG；原生菜单在其所属窗口的已观测屏幕边界内采集，以保留弹出层。没有缩放、拼接、重绘或替换界面内容。README 的显示宽度只影响页面预览，点击可查看原始文件。每张图片的尺寸、来源方法、UTC / 北京时间和 SHA-256 均保存在 [capture-manifest.json](capture-manifest.json)。

拍摄实例和临时 Everything 客户端已退出，原有正式 Velo 与 Everything 服务保留。验证范围与临时资源处理见上述本轮记录。

## 后续更新

使用待记录构建和独立演示数据，保持主题、缩放与应用集合一致，等待图标及过渡完成后采集窗口。更新图片时同步中英文 README、用户指南及采集清单，检查版本、场景、文字、隐私和校验值。不要提交用户配置、索引、历史、缓存或本地采集日志。

旧版界面截图随旧版源码保存在 [go 分支](https://github.com/HBLADEH/velo-launcher/tree/go/docs/images)；`docs/baselines/` 内的历史验证图片保留原始构建身份。
