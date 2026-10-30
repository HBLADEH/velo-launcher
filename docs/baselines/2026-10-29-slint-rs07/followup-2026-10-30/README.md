# RS-07：2026-10-30 协助补验

**结果：RS-07 在本机 Windows 11 x64 的待验项已补齐，状态更新为已验证。G2 继续不放行，RS-06 暂缓的实机项未改判。** 本次补齐两种渲染器的单次多文件 Explorer 拖放、普通 Explorer → 管理员目标的权限边界、管理员数据源 → 管理员目标的实际 OLE 拖放、管理员文件选择回退、返回搜索实际输入以及退出清理。

前序实现和首轮检查见 [RS-07 主记录](../README.md)。本轮用户负责实际跨窗口拖动、UAC 和管理员窗口操作，助手读取原生状态、日志并保存窗口截图；没有注入拖放消息，没有修改系统安全过滤、缩放或电源设置。

## 构建与环境

继续使用前轮两份 Release，Rust 和产品源码未改动；每份 exe 和原身份中的 49 个源码/脚本/资源指纹全部匹配。原有 fmt、两种 clippy、各 26 项 Rust 测试、各 20 项组件断言、Release/冒烟与 Go/Vue 回归结果继续对应这两份构建，见 [原检查](../checks.json)及 [本轮核对](checks.json)。没有因新增测试夹具重复整套未受影响的产品检查，也没有重测性能预算。

| 渲染器 | exe SHA-256 | 普通 / 管理员 PID |
| --- | --- | --- |
| Software | `2C26130F54BC6A38983A2F5C5BA7E75D2AFBE3199F4F65259731864FD6589D48` | 34700 / 9924 |
| FemtoVG | `2F12AFA94F84810F09D5F0D25F2CB54B69B2D5D1537964C94C2F996210404080` | 25668 / 2040 |

本机保持单屏 2560 × 1440、125%（120 DPI），工作区 `(0,0,2560,1380)`。源与目标权限由 `OpenProcessToken` / `GetTokenInformation` 独立核实：普通 Explorer 为 `elevated=false / integrity_rid=8192`，管理员 Velo 与管理员夹具为 `elevated=true / integrity_rid=12288`。具体 PID、HWND、托盘注册和权限快照见 [native-observations.json](native-observations.json)。

新增 [rs07-drag-source.cs](../../../../scripts/fixtures/rs07-drag-source.cs)作为独立测试窗口：固定提供三个合成文件，真实鼠标手势在自己的 `ItemDrag` 中调用 WinForms `DoDragDrop(DataFormats.FileDrop)`；记录源权限、数量及最终 `Copy` 效果，不操作其他窗口、不合成输入、不执行文件。使用 Windows 自带 .NET Framework C# 编译器 14.8.10001.0，编译及源码/exe 指纹见 [drag-source-build.json](drag-source-build.json)。管理员源 PID 9696 / 7388 分别用于 Software / FemtoVG，同权限行的证据范围是这个原生 OLE 夹具。

## 实测矩阵

三个测试文件为 `中文 程序.exe`、`记事本 快捷方式.lnk` 和 `无效 文件.txt`。每次使用全新独立实例与数据目录，热键为 `Ctrl+Alt+K`；始终只做会话预览。完整事件与原日志 SHA 见 [desktop-events.json](desktop-events.json)，逐步人工回复见 [manual-observations.json](manual-observations.json)。

| 数据源 → 目标 | Software | FemtoVG | 证据 |
| --- | --- | --- | --- |
| 普通 Explorer → 普通 Velo，一次拖三个文件 | 通过 | 通过 | 用户明确确认单次手势；每次均记录 3 个相邻 `source=ole_drop` 事件，合并为一个 `entries=3` 工作结果，最终为 `total=3 supported=2 invalid=1`；[Software](screenshots/software-medium-single-gesture-drop.jpg)、[FemtoVG](screenshots/femtovg-medium-single-gesture-drop.jpg) |
| 普通 Explorer → 管理员 Velo | Windows 正确阻止 | Windows 正确阻止 | 用户拖动后仍为 0 项，对应区间无接收事件；主窗口保持显示且有管理员边界及“选择文件”提示；[Software](screenshots/software-high-medium-source-blocked.jpg)、[FemtoVG](screenshots/femtovg-high-medium-source-blocked.jpg) |
| 管理员 OLE 夹具 → 管理员 Velo，一次拖三个文件 | 通过 | 通过 | 源/目标均核实为 High，源记录 `count=3 effect=Copy`；目标各接收一批 3 项、2 支持/1 无效；[FemtoVG 截图](screenshots/femtovg-high-high-source-drop.jpg)。Software 成功后的截图遇实体 Esc 停止，保留用户确认及两端原生日志 |
| 管理员 Velo 的原生文件选择回退 | 通过，实际选入 1 项 | 通过，同时选入 2 项 | 原生完成均 `action=1 error=false`，随后有 `source=picker` 和预览发布。Software 的原请求建议选两项，实际日志返回一项，因此按一项记录；FemtoVG 实际返回两项 |
| 管理员弹窗返回与搜索输入 | 通过 | 通过 | 用户返回搜索后输入 `code` 并确认有结果；两个事件流都记录 `utf8_bytes=4 focused=true native_focused=true visible=true composing=false`，随后正常退出 |
| 退出清理 | 通过 | 通过 | 四次 Velo 均有 `lifecycle_quit`，两次夹具有 `phase=closed`；最终所有测试进程、窗口和托盘端点均不存在，独立进程可重获并释放测试热键 |

文件选择回退实际在已有 3 项基础上分别发布 4 / 5 项；其选择路径与已有路径不同，按当前路径预览规则保留。这里验证回退、预览和焦点，同路径去重继续由前轮检查覆盖。`.lnk` 仍只是文件存在性预览，目标和参数解析按后续业务迁移实现。

窗口失焦时，导入模式均保留页面；所属弹窗前台时的原生检查继续记录 `owned_foreground=true importing=true native_dialog=true action=None`。普通模式下的搜索失焦隐藏、显隐后草稿及原生菜单/确认框检查沿用首轮证据。

## 完整性、清理与结论

- [checks.json](checks.json)验证 4 次 Velo 运行、2 次管理员数据源、两份未变化的构建与 [7 张截图](screenshots.json)；所有测量事件 `dropped=0`。源和目标事件间隔只用来与用户回复共同核对同一次手势的合并，不用于宣称呈现延迟或热呼出预算达标。
- 两次未完成的 UAC 启动在用户授权后重试，一次实体 Esc 中断在用户明确要求继续后恢复；见 [interruptions.json](interruptions.json)。取消与中断独立保留，没有计作产品功能失败，也没有补造缺少的截图。
- [cleanup.json](cleanup.json)确认 Velo、管理员夹具进程、主窗口、托盘端点与测试 Explorer 窗口均已关闭，`Ctrl+Alt+K` 重获及释放成功；未改系统设置、未读写产品数据。原始本地日志、夹具 exe 和数据副本保留在忽略目录 `.reasonix/rs07-followup-2026-10-30/`。

**RS-07 的本机待验项清零。G2 仍不放行：RS-06 的物理多屏/混合 DPI/拔插、休眠及 Shell 通知区实际点击等仍按用户先前指示暂缓，原有后台激活失败观察继续保留。** Windows 10、其他机器与第三方 IME 的兼容范围没有扩展；不进入 RS-08 业务迁移，不改变 RS-14 默认渲染器决策。

## 重现方式

继续使用 [主记录的正常实例参数](../README.md#重现入口)，普通和管理员运行使用不同的 `--instance-id` / `--data-dir`，顺序启动以避免热键冲突。管理员目标通过系统 RunAs 启动，由操作者处理 UAC。先从普通 Explorer 尝试拖入并核对 0 项和提示，再用已提权的数据源测试同权限。

夹具使用三个同名测试文件和一个不存在的日志目标；构建与调用示例在仓库根目录运行：

```powershell
$compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$fixtureSource = (Get-Item scripts/fixtures/rs07-drag-source.cs).FullName
$fixtureExe = Join-Path $PWD '.reasonix/rs07-drag-source.exe'
& $compiler /nologo /target:winexe /reference:System.Windows.Forms.dll /reference:System.Drawing.dll "/out:$fixtureExe" $fixtureSource
# 指定只含三个测试文件的目录，以及尚不存在的日志文件；UAC 由操作者处理。
Start-Process -FilePath $fixtureExe -Verb RunAs -WindowStyle Normal -ArgumentList '"D:\测试文件目录"', '"D:\测试输出\source-events.log"'
```

夹具界面 Ctrl+A 后用鼠标整批拖动；不要把夹具的程序化 OLE 数据构造当成鼠标注入。完成后测试文件选择、返回搜索的实际输入，使用底栏“退出”结束 Velo，并关闭夹具与测试 Explorer；重新核对原生端点及热键释放。
