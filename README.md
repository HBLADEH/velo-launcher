<p align="center">
  <img src="frontend/src/assets/logo.png" alt="Velo 标志" width="128" height="128" />
</p>

<h1 align="center">Velo</h1>
<p align="center"><strong>Fast. Light. Ready.</strong><br />按下快捷键，找到应用，即刻启动。</p>
<p align="center"><strong>简体中文</strong> · <a href="README.en.md">English</a></p>
<p align="center">
  <a href="https://github.com/HBLADEH/velo-launcher/actions/workflows/ci.yml"><img src="https://github.com/HBLADEH/velo-launcher/actions/workflows/ci.yml/badge.svg" alt="Windows 构建状态" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT 许可证" /></a>
  <img src="https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0078D4" alt="Windows 10 / 11" />
</p>
<p align="center">
  <a href="https://github.com/HBLADEH/velo-launcher/releases">下载</a> ·
  <a href="#快速上手">快速上手</a> ·
  <a href="docs/user-guide.md">用户指南</a> ·
  <a href="CONTRIBUTING.md">参与贡献</a> ·
  <a href="https://github.com/HBLADEH/velo-launcher/issues">问题反馈</a>
</p>

Velo 是面向 **Windows 10 / 11 x64** 的本地应用启动器，基于 Go、Wails 2、Vue 3 和 TypeScript 构建。通过全局快捷键唤起窗口，搜索应用、访问常用工具，用键盘完成日常启动操作。

> 当前版本为 **0.9.5 预览版**，发布文件暂未签名，性能与长期稳定性仍在验证中。目前仅支持 Windows，软件界面为简体中文。

## 界面预览

以下为 Velo 0.9.3 在 Windows 上的实际运行截图，展示中文界面。应用列表随本机安装的软件和使用记录变化。

<p align="center">
  <img src="docs/images/home-light.jpg" alt="Velo 浅色首页，展示常用应用与系统快捷入口" width="640" />
</p>
<p align="center"><em>首页：集中访问常用应用和 Windows 工具。</em></p>

| 搜索应用 | 外观设置 |
| --- | --- |
| ![输入 code 后的真实应用搜索结果](docs/images/search.jpg) | ![Velo 外观设置与主题选择](docs/images/appearance.jpg) |
| 输入关键词，选择结果即可启动。 | 支持浅色、深色和跟随系统主题。 |

## 主要功能

- **键盘操作**：全局快捷键唤起，方向键选择，Enter 启动；支持失焦隐藏和系统托盘。
- **应用搜索**：支持精确、前缀、子串、模糊匹配和单字符拼写容错，结合启动频率、最近使用和查询历史排序。
- **自动索引**：发现开始菜单、桌面、Windows Apps 和 Program Files 中的应用，可添加自定义扫描目录；默认合并重复项并隐藏卸载、帮助、更新等辅助入口。
- **快捷访问**：首页展示已固定应用、常用应用和系统快捷入口；系统快捷支持中文、英文、拼音及缩写搜索。
- **便捷操作**：清空回收站、关机、重启、锁屏；清空回收站及关机、重启前显示确认提示。
- **自定义应用**：拖入文件、选择文件或粘贴路径，添加扫描范围外的 `.exe` / `.lnk`；自定义应用与首页固定分别管理。
- **外观与偏好**：支持浅色、深色和跟随系统主题，可调整快捷键、登录启动、结果数量及搜索权重。
- **本地数据**：设置、索引和启动历史保存在本机；索引与图标使用缓存，默认每 30 分钟后台刷新。
- **应用更新**：从 GitHub Releases 检查版本，下载后校验 SHA-256，支持免安装版替换和安装版升级。

## 下载与安装

前往 [Releases](https://github.com/HBLADEH/velo-launcher/releases) 选择 Windows x64 发布文件：

| 方式 | 文件 | 使用方法 |
| --- | --- | --- |
| 安装版 | `*-installer.exe` | 运行安装向导 |
| 免安装版 | `velo-launcher.exe` | 放到固定目录后直接运行 |

两种方式均将设置和缓存保存到 `%LOCALAPPDATA%\Velo`。免安装版不会将数据保存在程序旁边；开启登录启动前，请先确定程序位置。

运行需要 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。如果系统缺少该组件，请先安装。目前不支持 macOS / Linux。

## 快速上手

1. 启动 Velo，等待首次应用索引完成。后续启动会优先读取缓存。
2. 按 `Alt+Space` 唤起窗口，输入应用名称，例如 `code`。
3. 用 `↑` / `↓` 选择结果，按 `Enter` 启动；点击星标可将应用固定到首页。
4. 按 `Ctrl+,` 打开设置，调整快捷键、主题和索引目录。

在 **设置 → 快捷键** 点击录制区域，直接按下组合键即可记录；点击“保存设置”后生效，`Esc` 可取消录制。

可开启“全屏时禁止快捷键呼出”，避免打断全屏应用；托盘入口仍然可用。每次呼出主页会立即聚焦搜索框，名称、关键词和别名均支持中间匹配。

| 操作 | 快捷键或入口 |
| --- | --- |
| 显示 / 隐藏启动器 | `Alt+Space`（可修改） |
| 选择上 / 下一个结果 | `↑` / `↓` |
| 启动选中应用 | `Enter`；默认也支持 `Space` |
| 清空输入；输入为空时隐藏窗口 | `Esc` |
| 打开设置 | `Ctrl+,` 或齿轮按钮 |
| 添加自定义应用 | 首页“＋ 自定义应用”或设置 → 索引 |
| 刷新索引 | 窗口底部刷新按钮或设置 → 索引 |
| 检查更新 | 设置 → 关于 |
| 完全退出 | 窗口底部“退出”或托盘右键菜单 |

默认情况下，空格键用于启动选中应用。如需输入 `visual studio` 等带空格的查询，请在 **设置 → 常规** 中关闭“按空格键启动选中应用”。

应用添加、固定、更新机制、数据目录及常见问题详见 [用户指南](docs/user-guide.md)。

## 开发与构建

推荐与 CI 保持一致：**Windows x64、Go 1.25.6、Node.js 22.15.0、npm 10、Wails CLI 2.12.0**，并安装 WebView2 Runtime。

```powershell
git clone https://github.com/HBLADEH/velo-launcher.git
cd velo-launcher
go install github.com/wailsapp/wails/v2/cmd/wails@v2.12.0
wails doctor
npm --prefix frontend ci
wails dev
```

验证与打包：

```powershell
# 格式检查、前端检查与构建、go vet、Go 测试及 Windows 打包
./scripts/verify.ps1

# 仅验证，不生成 Windows 可执行文件
./scripts/verify.ps1 -SkipBuild

# 同时生成安装包（需要 NSIS，且 makensis 位于 PATH 中）
./scripts/verify.ps1 -Installer
```

构建产物位于 `build/bin/`。GitHub Actions 使用同一验证脚本；`frontend/wailsjs/` 为 Wails 自动生成的绑定，请勿手工修改。

```text
frontend/src/   Vue 界面、样式与资源
internal/       搜索、索引、配置、历史与 Windows 平台实现
build/          应用图标、Windows 元数据与安装包配置
scripts/        验证与资源测量脚本
docs/           用户指南、开发说明与验证记录
```

## 文档

| 文档 | 内容 |
| --- | --- |
| [用户指南](docs/user-guide.md) | 自定义应用、更新、常见问题与数据存储 |
| [开发说明](docs/development.md) | 调试参数、测试、发布与资源测量 |
| [系统快捷入口](docs/system-shortcuts.md) | 支持的入口与 Windows 兼容性 |
| [构建资源](build/README.md) | 标志、程序图标与安装包资源 |
| [验证记录](docs/verification.md) | 已完成验证、实测结果及限制 |
| [实施清单](docs/implementation.md) | 功能进度与待验收项目 |

## 项目状态

Velo 当前专注于 Windows 本地应用搜索与启动，尚未提供插件或云端同步。主要功能已实现，完整性能与稳定性验收仍在进行。

历史测试中，包含 WebView2 子进程的私有内存约为 **154–218 MB**，尚未达到 80 MB 的设计目标。此数据是特定环境下的测量结果，不代表所有设备的资源占用；详见 [验证记录](docs/verification.md)。

## 参与贡献

欢迎提交缺陷报告、功能建议、文档改进和 Pull Request。开始前请阅读 [贡献指南](CONTRIBUTING.md)，了解开发验证流程和提交建议。

报告问题请前往 [Issues](https://github.com/HBLADEH/velo-launcher/issues)，附上 Windows 版本、Velo 版本、复现步骤、预期与实际结果。分享日志或截图前，请移除个人路径及其他敏感信息。

## 许可证与致谢

Velo 采用 [MIT License](LICENSE)。

界面使用 [Microsoft Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons)（MIT），以本地 SVG 子集打包；第三方应用保留原生图标。相关许可见 [图标许可证](frontend/src/assets/fluent/LICENSE.txt)，接入规范见 [图标说明](frontend/src/assets/fluent/README.md)。应用内“设置 → 关于”也提供第三方许可信息。
