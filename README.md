<p align="center">
  <img src="frontend/src/assets/logo.png" alt="Velo logo" width="160" height="160" />
</p>

<h1 align="center">Velo</h1>
<p align="center"><strong>Fast. Light. Ready.</strong><br />按下快捷键，找到应用，即刻启动。</p>
<p align="center">
  <a href="https://github.com/HBLADEH/velo-launcher/releases">下载预览版</a> ·
  <a href="#快速上手">快速上手</a> ·
  <a href="#开发与构建">开发与构建</a> ·
  <a href="https://github.com/HBLADEH/velo-launcher/issues">反馈问题</a>
</p>

Velo 是面向 **Windows 10 / 11 x64** 的本地应用启动器，基于 Go、Wails 2、Vue 3 和 TypeScript 构建。通过全局快捷键呼出，输入应用名称即可搜索和启动，让常用应用始终触手可及。

> 当前为 **0.9.0 预览版**。主要功能已实现，性能与长期稳定性仍在持续验证。发布文件暂未签名。

## 功能亮点

- **快速启动首页**：空搜索时展示已固定、常用应用和系统快捷；常用应用结合启动历史与应用来源排序。
- **自定义应用库**：通过拖入、选择文件或粘贴路径添加 `.exe` / `.lnk`，让扫描范围外的应用也能被搜索；支持单独删除，重启后仍保留。
- **自动更新**：启动后检查发布页，发现新版本时提示；便携版原地替换、安装版静默升级，下载内容用发布页的 SHA-256 校验。
- **键盘优先**：全局快捷键呼出，方向键选择，Enter 或空格启动；失焦自动隐藏，托盘常驻。
- **灵活搜索**：支持精确、前缀、子串、模糊匹配与单字符拼写容错，结合使用频率、最近启动和查询历史排序。
- **自动发现应用**：索引开始菜单、桌面、Windows Apps 和 Program Files，支持添加自定义目录。
- **更干净的结果**：默认隐藏卸载、帮助、更新等辅助入口，合并重复快捷方式，跳过深层组件程序。
- **本地运行**：设置、索引和启动历史保存在本机；使用索引与图标缓存，默认每 30 分钟后台刷新。
- **按习惯调整**：自定义快捷键、登录启动、结果数量和搜索权重，支持浅色、深色及跟随系统主题。

## 下载与安装

前往 [Releases](https://github.com/HBLADEH/velo-launcher/releases) 下载 Windows x64 版本：

- **安装包**：运行 `*-installer.exe`，按向导安装。
- **独立程序**：将 `velo-launcher.exe` 放到固定目录后直接运行；设置与缓存仍保存在用户本地数据目录。

运行需要 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。如果系统缺少该组件，请先安装。当前不支持 macOS / Linux。

## 快速上手

1. 启动 Velo，等待首次应用索引完成；后续启动会优先使用已有缓存。
2. 按 `Alt+Space` 呼出窗口，输入应用名称，例如 `code`。
3. 用 `↑` / `↓` 选择结果，按 `Enter` 启动。
4. 按 `Ctrl+,` 打开设置，按需要调整快捷键、主题和索引目录。

| 操作 | 快捷键或入口 |
| --- | --- |
| 显示 / 隐藏启动台 | `Alt+Space`（可修改） |
| 选择上 / 下一个结果 | `↑` / `↓` |
| 启动选中应用 | `Enter`；默认也支持 `Space` |
| 隐藏窗口 / 返回搜索 | `Esc` |
| 打开设置 | `Ctrl+,` 或齿轮按钮 |
| 刷新应用索引 | 窗口底部刷新按钮，或设置 → 索引 |
| 检查更新 | 设置 → 关于，或窗口底部的版本按钮 |
| 完全退出 | 窗口底部“退出”，或托盘右键菜单 |

开启“按空格键启动选中应用”后，空格用于启动；如需输入 `visual studio` 等带空格的查询，请在设置 → 常规中关闭此选项。

托盘图标左键单击可打开启动台，右键可打开设置或退出。再次运行程序会唤起已有实例。启用登录启动前，请先将独立程序放在固定位置。

## 添加搜索范围外的应用

点击首页的 **“＋ 自定义应用”**，或进入 **设置 → 索引 → 管理自定义应用**。可以拖入一个或多个 `.exe` / `.lnk` 文件、点击“选择文件”，或粘贴应用完整路径后点击“添加”。管理页面会保持显示，方便切换到资源管理器。

添加后立即加入搜索库，刷新索引或重启 Velo 后仍可搜索，无需把应用所在的整个目录加入扫描范围。列表中的“删除”移除该应用的自定义记录及首页固定，不删除原程序文件；若该应用也在自动扫描范围内，仍可通过自动索引找到。重复添加同一目标与参数不会产生重复项；程序移动后需要重新添加。

添加应用与首页固定独立：新添加的应用不会自动固定，搜索结果旁的 `☆` 可将应用固定到首页，取消固定后仍可搜索。自定义应用和固定记录统一保存在 `application-library.json` 中；旧版 `quick-launch.json` 中的手动应用会自动迁移，原文件保留为备份。

搜索优先考虑固定项、启动历史、桌面/开始菜单入口及常见应用，并降低更新服务等辅助程序的排序；精确名称匹配仍优先。常用排序只使用 Velo 的本地启动记录，不读取其他启动器的历史或固定列表。

内置系统快捷包括计算器、文件资源管理器、任务管理器、命令提示符、控制面板、卸载或更改程序、环境变量和设备管理器；支持中文名称及 `calc`、`cmd`、`path` 等关键词搜索。

## 更新 Velo

设置 → **关于** 中可以立即检查更新；默认也会在启动后自动检查一次，只读取 GitHub 发布页的版本信息，不上传任何本机数据。发现新版本时，窗口底部会出现版本按钮，打开即可查看发布说明、下载并安装。

点击“下载并安装”后 Velo 会退出，由独立脚本完成替换并自动重新启动：

- **独立程序**：直接覆盖当前可执行文件，随后重启。请确保程序放在你有写权限的位置。
- **安装版**：安装在 Program Files 时，改用安装包静默升级，Windows 会提示 UAC 确认。

下载内容会用发布页 `SHA256SUMS.txt` 中的 SHA-256 校验；校验文件缺失或不匹配时会停止自动安装，只保留手动下载。更新检查可以在设置 → 关于中关闭。

## 常见问题

**快捷键没有反应？**

快捷键可能被其他程序占用。通过托盘打开设置，在“快捷键”中更换组合；注册失败时界面会显示提示。

**找不到某个应用？**

在设置 → 索引中检查应用来源，添加应用所在目录，再刷新索引。默认支持 `.lnk` 和 `.exe`，并枚举 Windows Apps。Program Files 默认只扫描浅层主程序；如需辅助入口或深层组件，可关闭“隐藏卸载、帮助、更新等辅助项”。

**窗口隐藏后程序还在运行吗？**

是。正常模式不显示任务栏按钮，失焦或按 `Esc` 只会隐藏窗口；使用托盘菜单或窗口底部“退出”结束程序。

**数据存在哪里？**

位于 `%LOCALAPPDATA%\Velo`：

| 路径 | 用途 |
| --- | --- |
| `config.json` | 用户设置 |
| `index.json` | 应用索引与文件指纹缓存 |
| `history.json` | 启动次数、最近使用及查询选择历史 |
| `application-library.json` | 自定义搜索应用及独立的首页固定记录 |
| `cache/icons/` | 应用图标缓存 |
| `updates/` | 更新时下载的程序文件与替换脚本，完成后自动清理 |
| `logs/` | 当前及上一会话日志 |
| `webview/` | WebView2 本地运行数据 |

## 开发与构建

推荐使用与 CI 一致的环境：Windows x64、Go **1.25.6**、Node.js **22.15.0**、npm **10**、Wails CLI **2.12.0**，以及 WebView2 Runtime。

```powershell
git clone https://github.com/HBLADEH/velo-launcher.git
cd velo-launcher
go install github.com/wailsapp/wails/v2/cmd/wails@v2.12.0
wails doctor
npm --prefix frontend ci
wails dev
```

验证并生成 Windows 程序：

```powershell
./scripts/verify.ps1
# 仅验证，不打包
./scripts/verify.ps1 -SkipBuild
# 同时生成安装包（需安装 NSIS，并将 makensis 加入 PATH）
./scripts/verify.ps1 -Installer
```

产物输出到 `build/bin/`。验证脚本包含格式检查、Wails 绑定生成、前端 lint / 类型检查 / 构建、`go vet` 和 Go 测试；GitHub Actions 使用相同流程构建安装包。

```text
frontend/src/       Vue 界面、样式与品牌资源
internal/          搜索、索引、配置、历史及 Windows 平台能力
build/             应用图标、Windows 元数据与安装包配置
scripts/           验证与资源测量脚本
docs/              开发说明、实施清单与验证记录
```

Logo 源文件位于 [`frontend/src/assets/logo.png`](frontend/src/assets/logo.png)，用于应用界面和本文档；`build/appicon.png` 为打包副本，`build/windows/icon.ico` 为 Windows 图标，程序、托盘及安装包共用。替换与生成方式见 [构建资源说明](build/README.md)。

更多调试参数、集成测试、性能测量和构建注意事项见 [开发说明](docs/development.md)。`frontend/wailsjs` 由 Wails 自动生成，请勿手工修改。

## 项目状态与反馈

当前专注于 Windows 本地应用搜索与启动，尚未提供插件或云端同步。历史实测中，含 WebView2 子进程的私有内存约为 **154–218 MB**，80 MB 目标尚未达成；完整性能与稳定性验收仍在进行，详见 [验证记录](docs/verification.md) 和 [实施清单](docs/implementation.md)。

欢迎通过 [Issues](https://github.com/HBLADEH/velo-launcher/issues) 提交问题或建议。报告问题时请附上 Windows 版本、Velo 版本、复现步骤及相关日志片段；分享前请检查日志中的本地路径等个人信息。

## 许可证

本项目采用 [MIT License](LICENSE)。
