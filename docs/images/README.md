# 界面截图 · Screenshots

[返回项目首页](../../README.md)

这些图片截取自 Windows 上实际运行的 Velo 0.9.3，采集日期为 2026-09-28。使用本地 `build/bin/velo-launcher.exe --diagnostics`，以便截图时窗口不因失焦而隐藏。图片为窗口捕获工具输出的原始 JPEG，未经界面合成或内容替换。

| 文件 | 场景 |
| --- | --- |
| `home-light.jpg` | 浅色首页：常用应用与系统快捷入口 |
| `search.jpg` | 输入 `code` 后的应用搜索结果 |
| `appearance.jpg` | 设置 → 外观：主题选择 |

截图中的应用列表、结果顺序和索引提示来自本机环境，不是产品默认数据。底部“索引有 1 条提示”表示扫描期间遇到不可访问的目录，保留真实状态。截图未展示用户名、账户信息或个人文件；搜索页中可见应用安装路径。

更新截图时，运行待记录版本，等待索引和界面过渡动画结束，仅捕获应用窗口。检查路径、提示和列表中是否包含个人信息，再替换对应文件，并同步更新中英文 README 的说明。不要为了截图而提交用户配置或索引数据。

These are actual Windows application captures of Velo 0.9.3, taken on 2026-09-28 using diagnostics mode to prevent hiding on focus loss. Original JPEG captures are retained without compositing or replacing UI content. App lists and the index notice reflect the local machine; installation paths are visible in the search screenshot. Check for personal information and update both READMEs when replacing images.

## 配置备份预览（2026-10-04）

`config-backup-preview.jpg` 为当前源码的“设置 → 备份”页面在本地浏览器中的真实渲染截图，使用 640 × 660 页面尺寸。预览连接真实 Go 配置服务，配置与备份均位于独立测试目录，截图展示实际生成文件后的成功提示。此图用于验证页面布局及备份交互，不是原生桌面窗口截图，也不包含用户配置数据。

`config-backup-preview.jpg` captures the current settings backup UI in a local browser, connected to the real Go configuration service with isolated test data. It shows a successful backup at 640 × 660; it is a browser preview, not a native desktop-window capture.
