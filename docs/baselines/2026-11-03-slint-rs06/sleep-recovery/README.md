# RS-06：实际 S3 睡眠与恢复

执行日期：2026-11-03（北京时间）。**Software 与 FemtoVG 各一次实际 S3 睡眠/恢复通过。** 系统电源事件、同一进程/窗口、托盘、实际热键呼出和唤醒后的输入均有证据。临时电源设置已恢复，测试进程、窗口、托盘端点及热键已清理。

本记录延续[同日双屏补验](../README.md)及[前次补充记录](../followup/README.md)，范围为睡眠恢复及收尾时的真实托盘菜单。后续 [FemtoVG 实际拔插](../femtovg-unplug/README.md)已补齐最后一项，当前 RS-06 已验证，[G2 在已测范围放行](../g2-acceptance.md)；不把本轮睡眠日志当作拔插证据。

## 构建与测试条件

使用同日已完整验证的两份正常模式 Release，exe 哈希及各 49 个源码/脚本/资源指纹重新核对一致，未改应用源码、依赖或工具链，没有重新构建。完整 fmt/clippy、各 26 项 Rust 测试和各 20 项组件断言沿用[同日检查](../checks.json)，本项没有重采性能或内存。

| 渲染器 | SHA-256 | 本次 PID / HWND |
| --- | --- | --- |
| Software | `417FF956E78C632E101989586A6CACA30BF0DF7414E13122CC75A26B0338E993` | 225288 / `761456` |
| FemtoVG | `AD4A650FB62AD4A13FA12239C8CC13299748241FD0B26D1B557AEBAE08B7BCB3` | 219264 / `3F80DF8` |

系统为同一台 Windows 11 x64 双屏电脑，主屏 `(0,0)`、副屏 `(1920,42)`，均 1920 × 1080 / 96 DPI。两个实例先后运行，分别使用 `rs06-sleep-software` / `rs06-sleep-femtovg`、独立数据隔离路径和 `Ctrl+Alt+K`；未使用隔离生命周期或消息注入。唤醒和解锁由操作者完成，热键及字符输入通过 Computer Use 实际按键路径检查。

## 离开模式原因与临时配置

前次 Windows 电源菜单的睡眠请求只记录了 Kernel-Power 59“进入离开模式”，没有实际 S3。此次用 `powercfg /qh` 读到当前电源方案允许交流供电的离开模式（AC=1、DC=0）。管理员只读电源请求进一步确认 `icssvc` 服务存在 AWAYMODE 请求，见 [测试策略与请求](power-policy-test.json)。

操作者在管理员 PowerShell 执行本机准备的脚本，将原电源方案的交流“允许离开模式”临时从 1 改为 0，直流值仍为 0；原值预先保存于 [power-policy-original.json](power-policy-original.json)。没有停止服务、关闭其他应用、更改休眠支持或增加电源请求覆盖规则。

这项临时设置使两次请求都进入实际 S3。测试完成后，操作者执行恢复脚本，随后独立读取当前方案，确认原 AC=1、DC=0 已恢复，见 [power-policy-restored.json](power-policy-restored.json)。因此通过结论的环境前提是本次实际进入 S3；恢复原策略后，系统仍可能按原设置处理 `icssvc` 的离开模式请求，不将此环境行为归因于 Velo。

## 实际睡眠与恢复证据

| 渲染器 | 睡眠时间（北京时间） | 唤醒时间（北京时间） | 窗口与输入结果 |
| --- | --- | --- | --- |
| Software | 12:36:46.793 | 12:38:39.942 | PID/HWND、位置、96 DPI、查询 `code` 保留；托盘注册；实际 `Ctrl+Alt+K` 呼出并追加 `x` 成功 |
| FemtoVG | 12:51:16.670 | 12:53:43.932 | PID/HWND、位置、96 DPI、查询 `code` 保留；托盘注册；实际 `Ctrl+Alt+K` 呼出并追加 `x` 成功，内容正常重绘 |

两轮均观察到 Kernel-Power 42、107 和 Power-Troubleshooter 1；`TargetState=4`、`EffectiveState=4` 对应 Windows 的 S3 系统电源状态。表中时间取自 Power-Troubleshooter 的 `SleepTime` / `WakeTime`，原 UTC 值保留在 [Software 电源事件](software-power-events.json)与 [FemtoVG 电源事件](femtovg-power-events.json)。这些时间用于证明睡眠过程，不作为应用启动、呼出或恢复延迟指标。

睡眠前两窗口均位于副屏。唤醒后原生只读快照先确认窗口仍隐藏、托盘已注册、双屏/DPI/位置保持；恢复回调没有自行显示已隐藏窗口。随后从独立临时前台窗口实际按测试热键，原查询保留；无需点击搜索框，实际字符输入回调同时满足 `focused=true native_focused=true visible=true composing=false`。输入事件的 UTC 时间再次与系统 `WakeTime` 比较，确认发生在唤醒之后。

| 场景 | Software | FemtoVG |
| --- | --- | --- |
| 睡眠前 | [截图](screenshots/software-before-s3.jpg) | [截图](screenshots/femtovg-before-s3.jpg) |
| 唤醒后热键呼出 | [截图](screenshots/software-after-s3-hotkey.jpg) | [截图](screenshots/femtovg-after-s3-hotkey.jpg) |
| 唤醒后实际输入 | [截图](screenshots/software-after-s3-input.jpg) | [截图](screenshots/femtovg-after-s3-input.jpg) |

原生快照和指纹见 [native-observations.json](native-observations.json)，截图及 SHA-256 见 [screenshots.json](screenshots.json)，最终测量事件和原始日志指纹见 [Software](software-desktop-events.json)、[FemtoVG](femtovg-desktop-events.json)，全部事件 `dropped=0`。交叉核对结果见 [Software](software-evidence-check.json)、[FemtoVG](femtovg-evidence-check.json)，汇总见 [summary.json](summary.json)。原始本机文件位于忽略目录 `.reasonix/rs06-sleep-20261103/`。

## 正常退出与当前阶段状态

Software 通过界面底栏明确“退出”，日志确认 `lifecycle_quit` 和 `desktop_stopped tray_removed=true`。FemtoVG 恢复测试后，操作者从实际系统托盘右键菜单依次选择“设置…”和“退出”；日志确认 `action=2`、`reason=tray_settings native_active=true`、`action=3`、`lifecycle_quit` 和托盘移除，补齐之前缺少的实际入口证据。见 [final-user-actions.json](final-user-actions.json)和 FemtoVG 完整事件；这次没有使用 Esc 取消菜单。

最终没有 `velo-slint`、临时前台夹具、主窗口或桌面服务端点，测试热键可重新取得并释放。双屏原排列及 100% 缩放保持，电源策略原值已恢复，见 [cleanup.json](cleanup.json)。

本轮睡眠恢复通过后，后续 [FemtoVG 拔插](../femtovg-unplug/README.md)也已完成，RS-06/G2 当前结论见 [G2 验收](../g2-acceptance.md)。RS-08 未开始，默认渲染器未决。
