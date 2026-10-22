# 界面截图 · Screenshots

[返回项目首页](../../README.md) · [用户指南](../user-guide.md)

## 当前界面截图（2026-10-23）

本组截图由维护者提供，用于当前 Velo 0.9.10 版本的文档，展示 Windows 上的简体中文界面。所有图片均按原始 PNG 文件收录，仅统一文件名；未裁剪、缩放、合成或替换界面内容。原图宽度均为 800 像素，README 中的首页与用户指南按 640 像素宽度展示，双列预览由 GitHub 按可用宽度缩放，图片比例保持不变。

| 文件 | 原图尺寸 | 场景与用途 |
| --- | --- | --- |
| `home-light.png` | 800 × 825 | 浅色首页：常用应用与系统快捷，README 首页主图 |
| `home-dark.png` | 800 × 825 | 同一首页的深色主题，README 根据阅读环境选择深浅预览，并提供两张原图链接 |
| `search-list.png` | 800 × 735 | 搜索 `code` 的列表结果，显示应用名称与安装路径 |
| `search-grid.png` | 800 × 395 | 搜索 `qk` 的统一网格，同时包含清空回收站、Quicker 与 QuickerStarter |
| `context-menu.png` | 800 × 500 | 搜索 `vsc` 后对 Visual Studio Code 右键，展示候选项操作 |
| `appearance.png` | 800 × 825 | 设置 → 外观：跟随系统主题与网格排列选项 |
| `custom-apps.png` | 800 × 825 | 自定义应用管理：添加入口及四个已添加应用，用于用户指南 |
| `config-backup.png` | 800 × 825 | 设置 → 备份：手动备份入口与恢复说明，用于用户指南 |

应用列表、排序、系统快捷数量及底部“索引有 4 条提示”均来自维护者的实际环境，不是产品默认数据。扫描提示予以保留；仅凭截图不能判断每条提示的原因。搜索列表与自定义应用页保留真实应用安装路径。备份截图展示操作入口，没有执行成功提示，也未显示备份文件的用户路径。

中英文 README 共用同一组中文界面截图，英文页面提供对应的英文说明。已替换并移除原 README 使用的三张 0.9.3 JPEG 截图；下方历史验证图片继续保留其原有说明。

These maintainer-provided Windows captures were added on 2026-10-23 for the current Velo 0.9.10 documentation. The original 800-pixel-wide PNG files are retained without cropping, resizing, compositing, or UI replacement. Both READMEs share the Chinese UI captures. App lists, installation paths, and the four index notices reflect the actual local environment. The backup capture shows the available controls, not a completed backup.

## 更新方式

运行待记录版本，等待索引、图标加载和界面过渡动画完成，仅捕获应用窗口。若截图工具会导致失焦隐藏，可先从托盘完全退出 Velo，再使用本地程序的 `--diagnostics` 参数启动。

检查路径、提示和列表中是否包含个人信息。以原始 PNG 替换对应场景，保持比例，同步更新中英文 README、用户指南及本文件中的日期、尺寸和场景说明。不要为了截图提交用户配置、索引或使用记录。

## 历史验证资料

### 配置备份预览（2026-10-04）

`config-backup-preview.jpg` 为当时源码的“设置 → 备份”页面在本地浏览器中的真实渲染截图，使用 640 × 660 页面尺寸。预览连接真实 Go 配置服务，配置与备份均位于独立测试目录，截图展示实际生成文件后的成功提示。此图用于验证页面布局及备份交互，不是原生桌面窗口截图，也不包含用户配置数据。当前用户指南改用上方的 Windows 实机截图。

`config-backup-preview.jpg` is a historical browser capture connected to the real Go configuration service with isolated test data. It shows a successful backup at 640 × 660 and remains available as validation material.

### 空回收站信息提示（2026-10-14）

`empty-recycle-bin-notice.jpg` 为当时 Go 源码的空回收站信息提示，使用真实 Windows `MessageBoxW` 显示并捕获。原生提示函数在独立测试中调用，截图展示信息图标、“回收站已为空，无需清理。”及单个“确定”按钮；实测按空格关闭后正常返回。测试没有调用清空接口，也没有删除文件。

`empty-recycle-bin-notice.jpg` captures the Go informational message box with a single OK button. Pressing Space dismissed the notice successfully; the test did not invoke the empty operation or delete files.
