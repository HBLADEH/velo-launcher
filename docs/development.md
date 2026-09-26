# 开发与验证

[返回项目首页](../README.md)

以下命令均在项目根目录执行。

## 启动参数

- `--background`：后台启动；只有快捷键注册成功才隐藏窗口。
- `--diagnostics`：保留任务栏并关闭失焦隐藏，仅供桌面验证使用。

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
application-library.json 自定义搜索应用与首页固定记录
logs/velo.log           当前会话 JSON 日志
logs/velo.previous.log  上一会话日志
cache/icons/            按源文件指纹缓存的本地图标
updates/                更新下载的程序文件与替换脚本（完成后自动清理）
webview/                WebView2 本地运行数据
```

自动更新使用无窗口的 PowerShell 子进程；不要使用 `DETACHED_PROCESS`，Windows PowerShell 5.1 可能在进程创建成功后直接退出而不执行脚本。安装版会等待旧进程退出，并将 NSIS 安装目录指定为原目录。安装失败或取消 UAC 时保留安装包和脚本，错误写入 `updates/velo-update-<PID>.ps1.log`，便于排查。

JSON 保存使用同目录临时文件替换。默认每 30 分钟后台刷新，也可手动刷新；未变化的快捷方式复用解析结果。没有持续磁盘轮询。不可访问的目录显示提示，并保留相应已有索引以便重试。

`application-library.json` 保存自定义应用与首页固定，两者相互独立：删除自定义应用同时移除它的固定，取消固定不影响搜索。旧版 `quick-launch.json` 中的手动应用会在首次启动时迁移，原文件保留为备份；迁移后新文件为准，不再回读旧文件。

设置包括常规、快捷键、外观、搜索、索引。更改主题/搜索参数不触发目录扫描；配置保存失败时回滚快捷键，启动失败不计入历史。切换“隐藏辅助项”会立即重建内存索引，并在后台重扫以同时恢复或跳过 Program Files 的深层扫描范围。

## 发布流程

版本号有四处需要同步，`TestVersionMetadataMatchesBuild` 会直接校验：

- `internal/version/version.go` 的 `Number`：自动更新的比较基准，与 tag 一致并去掉前缀 `v`
- `wails.json` 的 `info.productVersion`：Windows 版本资源与安装包只接受数字版本，可省略预发布后缀
- `frontend/package.json` 与 `frontend/package-lock.json` 的 `version`

```powershell
# 1. 本地验证（不打包）
./scripts/verify.ps1 -SkipBuild
git add -A
git commit -m "chore(release): 准备 vX.Y.Z 版本元数据"
git push origin main
# 2. 打 tag；CI 在 windows-latest 上安装 NSIS，构建 exe 与安装包并生成 SHA256SUMS.txt
git tag vX.Y.Z
git push origin vX.Y.Z
# 3. 等 CI 完成后取回产物
gh run list --workflow ci.yml --limit 3
gh run download <run-id> --name velo-launcher-windows-amd64 --dir build/release
# 4. 创建发布（预览版用 --prerelease）；说明中写入变更与校验值
gh release create vX.Y.Z --prerelease --title "Velo vX.Y.Z" --notes-file docs/releases/vX.Y.Z.md build/release/*
```

自动更新读取 release 列表中最新的非草稿版本，项目至今只发布 prerelease，所以不能使用会忽略预发布版本的 `/releases/latest`。更新需要的资产固定为 `velo-launcher.exe`、`*-installer.exe` 与 `SHA256SUMS.txt`，缺一即拒绝自动安装。

## 进度与实测

完整范围和逐项待验收项见 [实施清单](implementation.md)，原始规划见 [计划书](../velo-launcher_README.md)。目前主要功能已实现并完成首轮自动化 / 桌面验证，**尚未完成全部性能和稳定性验收**。

已实际验证：`code` → Visual Studio Code → Enter 启动；从其他应用 Alt+Space 呼出；Ctrl+, 打开设置；主题保存及本地历史排序；开始菜单/桌面/Program Files/Windows Apps 索引和图标缓存；托盘图标创建与移除、`--background` 实例中托盘与快捷键共存；候选过滤、重名合并与深层限制后的本机索引（455 项缓存、402 项可见），以及开发模式下图标与设置加载的修复。

1000 应用搜索基准约 **0.014–0.069 ms**，每次 2–3 次分配。单次缓存启动记录约 470–523 ms（进程入口到前端缓存结果就绪，不等同于严格冷启动）。完整内存统计包含 WebView2 子进程；已测得私有内存约 154–218 MB，**80 MB 目标尚未达成**。详见 [验证记录](verification.md)。

资源测量命令（排除 Velo 启动的其他应用）：

```powershell
./scripts/measure-resources.ps1 -ProcessId <Velo进程ID> -Seconds 15
```

输出包含各进程内存明细。仅当 `CPUSampleValid` 为 true 时使用 CPU 百分比；采样期间进程集合变化会返回无效样本，应在运行稳定后重测。该脚本测量稳定运行期，不用于启动阶段 CPU 总量。

已发布的 exe 与安装包均未签名；没有插件、云端同步或其他 Future 范围功能。

## 许可证

[MIT](../LICENSE)
