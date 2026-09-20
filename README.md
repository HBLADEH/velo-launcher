# Velo

**Fast. Light. Ready.** 基于 Go + Wails 2 + Vue 3 + TypeScript 的 Windows 本地应用启动器。

## 使用

运行 `build/bin/velo-launcher.exe`，或者在项目根目录执行 `wails dev`。已发布预览版 [v0.8.0-beta.1](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.8.0-beta.1)（exe 与 NSIS 安装包，均未签名）。

- `Alt+Space`：显示 / 隐藏；快捷键被占用时会显示提示，可在设置中更换。
- 输入应用名称；`↑` / `↓` 选择，`Enter` 启动，`Esc` 隐藏。
- `Ctrl+,`：设置；底部“退出”完全退出程序。
- 通知区域（托盘）常驻 Velo 图标：左键单击打开启动台，右键菜单可打开启动台、设置或退出。
- 选中候选项后按空格可直接启动（默认开启，可在设置中关闭；开启后查询内不再输入空格）。
- 正常模式隐藏任务栏，失焦自动隐藏。再次运行 exe 可唤起已有实例。
- `--background`：后台启动；只有快捷键注册成功才隐藏。
- `--diagnostics`：供桌面验证使用，保留任务栏并关闭失焦隐藏；此模式不作为默认窗口行为的验收证据。

首次运行后台扫描，已有索引缓存会立即用于搜索。默认来源是开始菜单、桌面、Program Files、Program Files (x86) 和 Windows Apps；可在设置中添加目录或关闭 Program Files 扫描。Program Files 只索引浅层主程序（`<Program Files>\<厂商>\<应用>\app.exe`），其中的深层组件（如 `Git\usr\bin`、`Windows Kits\...\bin`）不会进入候选列表。

索引支持 `.lnk` / `.exe`，读取快捷方式的目标、参数、工作目录、描述和图标。启动快捷方式时保留 Windows Shell 语义。搜索完全在内存中完成，支持精确、前缀、词前缀、子串、模糊及单字符拼写容错；历史次数、最近启动和相同查询的选择参与排序。

候选列表默认隐藏“卸载 / 帮助 / 更新 / 安装”等辅助入口，合并同一个应用的多份重复快捷方式（例如用户开始菜单与公共开始菜单各一份），并跳过 Program Files 的深层组件；可用设置中的“隐藏辅助项”开关恢复完整列表。图标以 64 px 提取并缓存（优先取快捷方式目标程序的图标，避免 Shell 叠加的快捷方式箭头），在高 DPI 下仍然清晰。

## 开发环境

- Windows 10 / 11 x64；当前实测环境为 Windows 11 x64
- Go 1.25.6、Node.js 22.15.0 / npm 10
- Wails CLI 2.12.0、WebView2 Runtime

```powershell
go install github.com/wailsapp/wails/v2/cmd/wails@v2.12.0
wails doctor
npm --prefix frontend ci
wails dev
```

## 验证与构建

```powershell
./scripts/verify.ps1
# 仅检查，不重新打包：
./scripts/verify.ps1 -SkipBuild
# 内存搜索基准：
go test ./internal/search -bench . -benchmem
# 本机 Windows Apps 集成测试（只枚举和读取图标，不启动）：
$env:VELO_INTEGRATION = '1'
go test ./internal/platform -run 'Test.*Icon' -v
# 通知区域托盘集成测试（会在通知区域短暂显示 Velo 图标）：
go test ./internal/platform -run TestTray -v
```

验证脚本执行 gofmt 检查、前端 lint / typecheck / build、Wails 绑定生成、go vet、Go 测试和 Windows x64 打包；失败即停止。加 `-Installer` 时改用 `wails build -nsis`，额外生成安装包（需要 `makensis` 在 PATH 中）。前端构建先于 Go 检查，因为 `//go:embed all:frontend/dist` 要求产物已存在。GitHub Actions 使用相同脚本（`-Installer`）并上传 exe 与安装包，远端 CI 已运行通过。

`wails build -nsis` 会把 `build/windows/installer/wails_tools.nsh` 的模板占位符就地渲染成当前 `wails.json` 元数据，因此本地构建后该文件会显示为已修改；请用 `git restore build/windows/installer/wails_tools.nsh` 恢复模板状态，不要提交渲染结果（否则安装包版本不再跟随配置）。

Wails 绑定位于 `frontend/wailsjs`，由工具生成，请勿手工修改。`main_bindings.go` 使用独立构建标签，生成绑定时不会访问运行中的用户数据。仅 Windows 实现当前桌面能力；未宣称支持 macOS / Linux。

## 本地数据

`%LOCALAPPDATA%\Velo`：

```text
config.json             设置（默认值补齐、版本检查、损坏备份恢复）
index.json              索引与文件指纹缓存
history.json            启动次数、最近使用、query → app
logs/velo.log           当前会话 JSON 日志
logs/velo.previous.log  上一会话日志
cache/icons/            按源文件指纹缓存的本地图标
webview/                WebView2 本地运行数据
```

JSON 保存使用同目录临时文件替换。默认每 30 分钟后台刷新，也可手动刷新；未变化的快捷方式复用解析结果。没有持续磁盘轮询。不可访问的目录显示提示，并保留相应已有索引以便重试。

设置包括常规、快捷键、外观、搜索、索引。更改主题/搜索参数不触发目录扫描；配置保存失败时回滚快捷键，启动失败不计入历史。切换“隐藏辅助项”会立即重建内存索引，并在后台重扫以同时恢复或跳过 Program Files 的深层扫描范围。

## 进度与实测

完整范围和逐项待验收项见 [实施清单](docs/implementation.md)，原始规划见 [计划书](velo-launcher_README.md)。目前主要功能已实现并完成首轮自动化 / 桌面验证，**尚未完成全部性能和稳定性验收**。

已实际验证：`code` → Visual Studio Code → Enter 启动；从其他应用 Alt+Space 呼出；Ctrl+, 打开设置；主题保存及本地历史排序；开始菜单/桌面/Program Files/Windows Apps 索引和图标缓存。本机索引 1665 项。

1000 应用搜索基准约 **0.014–0.069 ms**，每次 2–3 次分配。单次缓存启动记录约 470–523 ms（进程入口到前端缓存结果就绪，不等同于严格冷启动）。完整内存统计包含 WebView2 子进程；已测得私有内存约 154–218 MB，**80 MB 目标尚未达成**。详见 [验证记录](docs/verification.md)。

资源测量命令（排除 Velo 启动的其他应用）：

```powershell
./scripts/measure-resources.ps1 -ProcessId <Velo进程ID> -Seconds 15
```

输出包含各进程内存明细。仅当 `CPUSampleValid` 为 true 时使用 CPU 百分比；采样期间进程集合变化会返回无效样本，应在运行稳定后重测。该脚本测量稳定运行期，不用于启动阶段 CPU 总量。

已发布的 exe 与安装包均未签名；没有插件、云端同步或其他 Future 范围功能。

## 许可证

[MIT](LICENSE)
