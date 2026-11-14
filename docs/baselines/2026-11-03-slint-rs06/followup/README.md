# RS-06：恢复验证后的补充记录

日期：2026-11-03（北京时间）。延续[同日双屏补验](../README.md)，使用相同两份已验证 exe；重新核对 exe SHA-256 与各 49 个源码指纹，见 [build-check.json](build-check.json)。本次没有修改应用源码或重新构建，沿用同日已通过的完整自动检查。

后续进展：[实际 S3](../sleep-recovery/README.md)和 [FemtoVG 最后拔插](../femtovg-unplug/README.md)均已补齐，原电源值与显示配置已恢复，当前 [G2 在已测范围放行](../g2-acceptance.md)。下面保留本次早先离开模式尝试的原始证据与当时状态。

**本次会话结束时：Software 的副屏全新启动、缺失托盘入口与物理拔插已补齐；当时实际 S3 和 FemtoVG 剩余项目尚未验证。** 操作者请求睡眠后，Windows 实际记录进入“离开模式”。随后物理 Esc 停止 Computer Use，未继续进行界面输入。

## 已补齐的行为

| 项目 | 实际操作与结果 | 证据 |
| --- | --- | --- |
| 显示配置恢复 | 恢复前两屏已是 96 DPI，副屏位于 `(1920,50)`；恢复原位置后，原生探针确认主屏 `(0,0)`、副屏 `(1920,42)`、两屏均 96 DPI。恢复位置的系统调用仅用于收尾 | [原生快照](native-observations.json) |
| Software 副屏全新启动 | 副屏资源管理器实际双击测试快捷方式；正常模式独立实例 `rs06-followup-software`、独立数据目录与 `Ctrl+Alt+K`。PID 116896 / HWND `18B00BC` 首次位于副屏 `(2560,248)`，640 × 660；工具输入之前已记录原生焦点为 true。未点击搜索框即可输入 `code`，高度变为 264 | [首次输入](screenshots/software-secondary-first-input.jpg)、[启动摘录](software-startup.json) |
| 实际托盘设置 | 用户从系统通知区右键菜单点击“设置…”；日志收到 `action=2`，随后 `reason=tray_settings native_active=true appearance_open=true`，查询保留。没有注入 Shell 通知 | [设置页](screenshots/software-actual-tray-settings.jpg)、[完整事件](desktop-events.json) |
| 实际托盘打开与取消 | 用户左键实际图标呼出，并右键菜单后按 Esc 取消；分别观察到 `action=1`、`reason=tray native_active=true` 与 `action=4`。菜单活动期间所属前台关系使失焦检查为 `action=None` | [用户操作](human-actions.json)、[完整事件](desktop-events.json) |
| 物理拔出副屏 | 在原窗口实际拖回右侧副屏后，用户确认实际拔下视频线并保持断开。只读快照由两屏变为仅主屏；同一 PID/HWND 移回主屏 `(0,247)`，托盘仍注册。实际热键呼出保留查询，追加字符成功，原生/组件焦点均 true | [拔出前](screenshots/software-before-physical-unplug.jpg)、[拔出后输入](screenshots/software-physical-unplug-recall-input.jpg)、原生快照 |
| 物理插回副屏 | 用户插回视频线并确认双屏恢复；原 PID/HWND 与查询保留，工作区恢复到原配置。热键可呼出，实际重新输入 `code` 成功，高度为 264 | [插回后的输入](screenshots/software-replug-before-sleep-query.jpg)、原生快照 |

完整测量日志固定为 129 条事件，全部 `dropped=0`，SHA-256 与原文见 [desktop-events.json](desktop-events.json)。13 份只读原生快照见 [native-observations.json](native-observations.json)，6 张窗口截图与指纹见 [screenshots.json](screenshots.json)。原始文件保留在本机忽略目录 `.reasonix/rs06-followup-20261103/`。

Windows 重新连接显示器时曾短暂报告尚未扣除任务栏的工作区，后续设置通知将其收敛到 1032 物理像素；最终快照和窗口尺寸正常。没有用这一短暂状态冒充稳定工作区。

## 睡眠尝试的实际边界

机器报告支持 S3，休眠未启用，见 [power-capabilities.txt](power-capabilities.txt)。用户按要求从电源菜单请求睡眠，并回复“托盘操作完成，已实际睡眠并唤醒解锁”。System 日志在 `12:15:19 +08:00` 实际记录：

- Kernel-Power 187：用户模式进程尝试通过电源 API 更改系统状态。
- Kernel-Power 59：**系统正在进入“离开模式”。**

见 [电源请求事件](software-power-request-details.json)。从本次睡眠请求开始查询，未取得 Kernel-Power 42/107/506/507 或 Power-Troubleshooter 1；System 日志启用且可读取，见 [电源证据复核](software-sleep-evidence-review.json)与 [查询结果](software-power-events.json)。因此本次不能记为实际 S3 睡眠/恢复通过。

后续桌面检查返回“Computer Use was stopped by the user with the physical Escape key”，本轮未再发送任何界面输入，睡眠后的热键和输入检查也未执行。菜单取消所需的物理 Esc 与桌面工具的停止信号会冲突，下次需调整人工操作和采集顺序。

只读 `powercfg /requests` 查询要求管理员权限，当前未取得调用方列表；离开模式策略查询也未返回具体取值。见 [电源诊断](power-diagnostics.json)。本轮没有更改电源策略、休眠设置或请求覆盖规则；下一轮先查明实际睡眠为何进入离开模式，再重做真实 S3 验收。

## 清理与剩余项目

工具停止后通过终端核对路径和 PID，仅结束本轮 Software 与临时前台夹具进程；这是测试清理，不记为本次托盘“退出”操作通过。最终没有原型窗口、桌面服务端点或 `velo-slint` 进程，`Ctrl+Alt+K` 可重新取得并释放；双屏原排列和 100% 缩放保留。见 [cleanup.json](cleanup.json)。本轮测试快捷方式的资源管理器窗口保留，停止后未继续操作界面。

本次会话结束时的剩余项目（后续均已完成）：

1. 两种渲染器的真实 S3 睡眠/恢复与恢复后实际输入。
2. FemtoVG 的物理拔插/重新连接，以及实际托盘“设置…”和“退出”事件。Software 的缺失托盘入口已由本轮补齐，其实际托盘退出沿用同日早先已确认的证据。

保留前序完整检查和历史结果的原构建身份；RS-08 未开始，默认渲染器未决。
