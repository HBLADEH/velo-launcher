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
# 回收站检测集成测试（只查询，不执行清空）：
go test ./internal/platform -run TestEmptyRecycleBinNativeQuery -v
# 通知区域托盘集成测试（会在通知区域短暂显示 Velo 图标）：
go test ./internal/platform -run TestTray -v
```

验证脚本执行测量脚本回归检查、gofmt 检查、前端 lint / 按键解析测试 / typecheck / build、Wails 绑定生成、go vet、Go 测试和 Windows x64 打包；失败即停止。加 `-Installer` 时改用 `wails build -nsis`，额外生成安装包（需要 `makensis` 在 PATH 中）。前端构建先于 Go 检查，因为 `//go:embed all:frontend/dist` 要求产物已存在。GitHub Actions 使用相同脚本（`-Installer`）并上传 exe 与安装包，远端 CI 已运行通过。

`wails build -nsis` 会把 `build/windows/installer/wails_tools.nsh` 的模板占位符就地渲染成当前 `wails.json` 元数据，因此本地构建后该文件会显示为已修改；请用 `git restore build/windows/installer/wails_tools.nsh` 恢复模板状态，不要提交渲染结果（否则安装包版本不再跟随配置）。

Wails 绑定位于 `frontend/wailsjs`，由工具生成，请勿手工修改。`main_bindings.go` 使用独立构建标签，生成绑定时不会访问运行中的用户数据。仅 Windows 实现当前桌面能力；未宣称支持 macOS / Linux。

## 本地数据

`%LOCALAPPDATA%\Velo`：

```text
config.json             设置（默认值补齐、版本检查、损坏备份恢复）
backups/                手动及更新前的配置原文备份（不随更新下载清理）
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

`launcher:hidden` / `launcher:shown` 只更新可见性，不卸载页面或重置查询；普通和管理员启动指令发出后立即清空查询、搜索结果和选中状态，无需等待启动完成或系统确认框关闭。启动失败显示错误，不恢复旧查询；启动完成也不会清空等待期间新输入的查询。显示入口在原生窗口定位后通过 Wails UI 线程恢复 WebView 焦点；前端对延迟激活进行有界重试，在隐藏、失焦和卸载时取消，设置、导入和右键菜单打开时不抢占输入焦点。更新控制器由前端根组件持有，后端同时串行化安装请求，避免面板重新打开后重复下载。窗口顶部使用 Wails 的 `--wails-draggable` 区域，交互控件显式禁用拖动；首次显示按鼠标所在屏幕定位，此后显示与尺寸变化保留原位置，并按工作区域约束边界。位置仅保留在当前进程内，不写入用户配置。

清空回收站先调用 `SHQueryRecycleBinW` 查询所有驱动器；没有项目时显示“回收站已为空，无需清理。”的信息提示，仅有默认聚焦的“确定”按钮，可用空格或 Enter 关闭，不调用清空接口。有项目时保留 Shell 自带的确认提示。查询结构 `SHQUERYRBINFO` 在 32 位 Windows 中为 20 字节，64 位中为 24 字节，需与程序架构匹配。原生集成测试使用实际 Go 查询接口验证布局及空回收站分支，提示框使用替身，只查询，不删除文件。

JSON 保存使用同目录临时文件替换。默认每 30 分钟后台刷新，也可手动刷新；未变化的快捷方式复用解析结果。没有持续磁盘轮询。不可访问的目录显示提示，并保留相应已有索引以便重试。

`application-library.json` 保存自定义应用与首页固定，两者相互独立：删除自定义应用同时移除它的固定，取消固定不影响搜索。旧版 `quick-launch.json` 中的手动应用会在首次启动时迁移，原文件保留为备份；迁移后新文件为准，不再回读旧文件。

设置包括常规、快捷键、外观、搜索、索引、备份和关于。备份与设置保存共享提交锁，复制磁盘中的 `config.json` 原文，保留未知字段及格式；更新下载校验通过后必须备份成功，才生成和运行替换脚本。读取有效的旧配置只在内存中补齐新字段，不改写配置文件。更改主题/搜索参数不触发目录扫描；配置保存失败时回滚快捷键，启动失败不计入历史。切换“隐藏辅助项”会立即重建内存索引，并在后台重扫以同时恢复或跳过 Program Files 的深层扫描范围。

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

API 返回限流 `403` / `429` 后，更新器在 `Retry-After` / `X-RateLimit-Reset` 指定的时间前暂停 API 请求（缺少有效时间时暂停 5 分钟），改读同仓库的 `releases.atom`，按 tag 比较版本，并从选中版本的 `releases/expanded_assets/<tag>` 提取实际下载链接。备用通道复用原 HTTP 客户端与系统代理，不携带 API Token；仍由原安装流程强制校验 `SHA256SUMS.txt`。

## 进度与实测

完整范围和逐项待验收项见 [实施清单](implementation.md)，原始规划见 [计划书](../velo-launcher_README.md)。目前主要功能已实现并完成首轮自动化 / 桌面验证，**尚未完成全部性能和稳定性验收**。

已实际验证：`code` → Visual Studio Code → Enter 启动；从其他应用 Alt+Space 呼出；Ctrl+, 打开设置；主题保存及本地历史排序；开始菜单/桌面/Program Files/Windows Apps 索引和图标缓存；托盘图标创建与移除、`--background` 实例中托盘与快捷键共存；候选过滤、重名合并与深层限制后的本机索引（455 项缓存、402 项可见），以及开发模式下图标与设置加载的修复。

1000 应用搜索基准约 **0.014–0.069 ms**，每次 2–3 次分配。单次缓存启动记录约 470–523 ms（进程入口到前端缓存结果就绪，不等同于严格冷启动）。完整内存统计包含 WebView2 子进程；已测得私有内存约 154–218 MB，**80 MB 目标尚未达成**。详见 [验证记录](verification.md)。

### 轻量化测量与回归基线（OPT-00）

执行状态与原始证据见 [2026-10-27 基线](baselines/2026-10-27-opt00/README.md) 和[优化计划](lightweight-optimization-plan.md)。安装版与候选版分别记录；相同的 `0.9.10` 版本号不能证明它们是同一个构建。

资源采集前，先通过当前会话的 `index refreshed` 成功日志和界面扫描状态确认刷新已经结束，保持界面空闲，避开下一次定时刷新。采样期间不要同时构建、运行 benchmark、操作 Velo 或启动其他应用。确认数据目录属于目标进程，不能仅修改 `LOCALAPPDATA` 就认为实现了隔离：单实例互斥锁、窗口类和全局快捷键仍然共享。候选没有实际启动时，运行数据保持空缺。

```powershell
Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%'" |
    Select-Object ProcessId, Name, ExecutablePath

# 根据上一步结果填写，核实 exe 路径；证据目录需要预先创建。
$veloProcessId = 12345
$veloExecutable = 'C:\Program Files\HBLADEH\Velo\velo-launcher.exe'
$veloEvidenceDir = '.reasonix/opt00-next-run'
New-Item -ItemType Directory -Path $veloEvidenceDir -Force | Out-Null

# 可见且稳定后执行。IdleEvidence 填写本次确认扫描结束的具体依据。
./scripts/measure-resources.ps1 -ProcessId $veloProcessId -Seconds 15 -Samples 3 `
    -Edition Installed -ExpectedExecutable $veloExecutable -WindowState Visible `
    -IdleConfirmed -IdleEvidence '填写本次扫描完成时间及确认方式' `
    -OutputPath "$veloEvidenceDir/installed-visible.json"

# 刚隐藏后执行；确认期间保持隐藏，再顺序取得三组 15 秒样本。
./scripts/measure-resources.ps1 -ProcessId $veloProcessId -Seconds 15 -Samples 3 `
    -Edition Installed -ExpectedExecutable $veloExecutable -WindowState Hidden `
    -MinimumStateSeconds 30 -IdleConfirmed -IdleEvidence '填写本次扫描完成时间及确认方式' `
    -OutputPath "$veloEvidenceDir/installed-hidden-30s.json"

# 重新呼出并隐藏后单独执行，不能把进程存活时间当作隐藏时间。
./scripts/measure-resources.ps1 -ProcessId $veloProcessId -Seconds 15 -Samples 3 `
    -Edition Installed -ExpectedExecutable $veloExecutable -WindowState Hidden `
    -MinimumStateSeconds 300 -IdleConfirmed -IdleEvidence '填写本次扫描完成时间及确认方式' `
    -OutputPath "$veloEvidenceDir/installed-hidden-5m.json"
```

候选使用 `-Edition Candidate -ExpectedExecutable <候选实际路径> -DataDirectory <候选数据目录>`，在安装版退出或独立测试会话中执行，并确认实际根进程路径。对照时核对配置、索引、历史、应用库的 SHA-256、可见索引数量、DPI、WebView2 版本和生产 / 诊断模式。只使用同一套输入；保存的配置缺失字段由对应程序版本补默认值，不能把 `SavedConfig` 当作完整的实时设置查询。`--diagnostics` 与默认生产行为分组记录。

输出为 `SchemaVersion=2` JSON，含程序路径 / SHA-256 / 字节数 / 固定文件版本、代码与工具链信息、配置与数据指纹、各进程内存、CPU 和逐次窗口观察。`Environment.WorkspaceCommit` 是采集器工作区的提交；程序没有嵌入 `vcs.revision` 时，`Executable.SourceCommit` 默认空缺。只有通过发布清单 SHA-256 或构建记录确认提交后，才填写完整的 `-ExecutableCommit`，并在测量记录中附依据。当前 PATH 中的 Node / Wails CLI 是采集环境信息，不代表安装版当初使用的构建工具；程序的 Go / Wails 依赖另从 exe 的 Go build info 读取。

采集器只读枚举属于目标 PID 的 `VeloLauncherWindow`，记录 `IsWindowVisible`、DPI 与缩放比例；每约 1 秒检查一次进程树和窗口。状态缺失或采样中发生显隐切换时输出 `WindowState=Unknown`（状态未确认）；预期状态不符也不得纳入对照。`MinimumStateSeconds` 是从采集器开始观察起的持续状态下限，不是隐藏动作的精确时间；必须记录是否从一次新的隐藏动作开始。三个连续样本会在该下限之后依次取得，不能都写成“恰好隐藏 30 秒”。

逐样本检查 `CPUSampleValid`、`WindowStateMatches`、`StateContinuouslyObserved`、`DataStable`、`IdleConfirmed` 与 `BaselineEligible`，同时核对报告的 `ExecutableFileStable`。数据文件或刷新日志改变会使对应空闲样本无效；保留无效样本，另取新文件补测。`AllSamplesEligible` 仅描述该文件的全部样本是否可用，不表示 OPT-00 或性能目标验收通过。`-IdleConfirmed` 需要填写 `-IdleEvidence`，默认不宣称已经空闲；日志中的上次刷新数量也不是实时扫描状态。脚本不会覆盖已有输出。

PrivateBytes 为私有提交，WorkingSet 为全进程树工作集求和（共享页重复统计），单位 `MB` 字段按 1024² 字节换算，原始字节同时保留。CPU 用两端累计计数之差、单调时钟的实际秒数及逻辑 CPU 数归一化；检查每个轮询点的 PID / 启动时间和计数，变化或计数下降则 CPU 为 null。轮询仍可能漏掉完全发生在间隔内的短命进程或显隐变化，所以这些数值用于稳定空闲期，不能证明启动 CPU、物理常驻 <80 MB 或热呼出 <50 ms。CPU 显示 0 是该精度下的结果。

搜索内核与服务基准串行运行，至少三轮，不与资源采样重叠：

```powershell
go test ./internal/search -run '^$' -bench BenchmarkSearch1000Apps -benchmem -benchtime=300ms -count=3
go test ./internal/app -run '^$' -bench '^BenchmarkService(Home|Search)$' -benchmem -benchtime=300ms -count=3
```

服务夹具固定为 100 / 1000 / 5000 项、4 个固定应用和 1 个固定系统快捷，系统快捷总数固定为 3；分别无历史和所有条目有历史（过去约 24 小时、不同启动次数及 query 频次）。通过真实 `Service.New` 加载临时数据，计时只覆盖 `Home` / `Search` 调用，包含历史计算、锁、排名和混合结果；构建夹具、磁盘扫描、IPC、DOM 渲染不在计时内。服务夹具包含中文名称，不能把它与原有内核夹具的差值全部归因为服务开销。

### 桌面启动与热呼出计时

已提供仅 `latencyprobe` 标签启用的观察探针、`build-latency-probe.ps1`、`collect-latency.ps1` 和需在正常交互桌面运行的 `run-desktop-latency.ps1`。当前校准结果、时钟 / 原生焦点限制及三次暖启动、20 次真实按键、独立冷启动的采集步骤见[计时采集记录](baselines/2026-10-27-opt00-latency/README.md)。普通生产构建不包含探针；工具通过测试不代表已有有效计时数据。

真正冷启动、暖启动、热呼出分组保存，使用[空白 CSV](baselines/2026-10-27-opt00/desktop-latency.csv) 的列名另建本次证据文件。每行关联 exe SHA-256、环境 / 数据指纹记录、时间、测量方法与原始证据。`Valid=false` 的失败尝试保留但不参与分位数；超时和输入失败另报数量，不得通过删除慢样本改善结果。

- `ColdStartup`：`Boundary=ProcessStartToEditable`。独立重启 / 受控冷条件后的首次启动；在 `BootId` 填写该条件的证据 ID。后续重复启动属于暖启动。同一个条件不得重复计作冷样本；不要为了测量自动重启用户电脑或改动自启动设置。

  重启前须核对实际登录启动入口及其启用状态，不能只检查 `config.json` 的 `launch_at_startup`。2026-10-27 复核发现配置为 false，但注册表 `Run\Velo` 和 StartupApproved 仍启用，安装版已先于采样启动。具体证据与需用户明确授权才执行的临时停用 / 恢复步骤见[重启预检记录](baselines/2026-10-27-opt00-cold-preflight/README.md)；管理脚本默认 `Status` 只读，备份不等于更改启动项。
- `WarmStartup`：同样测从进程开始到搜索框可输入，保留 OS / WebView / Velo 索引缓存，正常退出后重复启动；建议至少 3 次，独立于冷启动。
- `HotkeyActivation`：`Boundary=KeyDownToEditable`。生产模式隐藏后，从实际快捷键按下到搜索框成功接受输入；至少 20 个有效样本。使用能够覆盖整个边界的桌面计时 / 高帧率录屏及输入证据，并注明分辨率、时钟和观察误差。分别保留隐藏等待时长及焦点、输入失败，计时后清空测试查询，不执行候选。

启动日志 `frontend interactive` 只作补充，不能代替上述可输入时刻。工具调用耗时和两次截图间隔也不能证明 <50 ms。尚无这些测量能力或实际样本时，CSV 只保留表头，延迟与分位数保持空缺。

```powershell
# 对真实桌面记录计算中位数和 P95；不会自动产生计时样本。
./scripts/summarize-latency.ps1 -InputPath <本次桌面CSV路径> -OutputPath <新JSON路径>
# 独立验证采集判定及统计规则（不操作桌面；亦已纳入 verify.ps1）：
./scripts/test-resource-measurement.ps1
./scripts/test-latency-summary.ps1
```

统计按程序 SHA-256、环境、输入、方法及指标分组；中位数在偶数样本时取中间两值均值，P95 使用最近秩 `ceil(0.95 × n)`。汇总器检查热呼出 20 次、启动每组 3 次的样本数量，拒绝有效记录中的非有限 / 非正耗时及重复冷条件。样本数量满足不等于性能达标，原始证据和真实冷条件仍需核验。

简要资源探针仍可使用：

```powershell
./scripts/measure-resources.ps1 -ProcessId <Velo进程ID> -Seconds 15
```

未提供空闲确认时，探针的 `BaselineEligible=false`；它用于检查目标进程与状态，不能直接作为已验收的空闲基线。

已发布的 exe 与安装包均未签名；没有插件、云端同步或其他 Future 范围功能。

## 许可证

[MIT](../LICENSE)
