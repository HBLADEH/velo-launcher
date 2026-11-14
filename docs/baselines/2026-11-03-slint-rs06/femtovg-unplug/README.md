# RS-06：FemtoVG 物理拔插与重新连接

执行日期：2026-11-03（北京时间）。**最后一项 FemtoVG 物理拔插/重新连接验证通过。** 操作者实际拔下、插回右侧副屏的视频线，原生显示器数量按 2 → 1 → 2 变化；同一进程、HWND、DPI、查询及托盘保留，断开和重新连接后均能实际热键呼出、输入并正常绘制。正常退出后无遗留实例、端点或测试热键。

本记录与[双屏/混合 DPI](../README.md)、[Software 拔插](../followup/README.md)、[两种实际 S3 恢复](../sleep-recovery/README.md)共同完成 RS-06。完整第二阶段结论见 [G2 验收](../g2-acceptance.md)，RS-08 尚未开始。

## 构建与环境

继续使用同日已完整验证的 FemtoVG Release：SHA-256 `AD4A650FB62AD4A13FA12239C8CC13299748241FD0B26D1B557AEBAE08B7BCB3`、10,362,880 字节。启动前重新核对 exe 与 49 个源码/脚本/资源指纹，全部一致，见 [build-check.json](build-check.json)。未改 Rust 应用、依赖、验证脚本或工具链；沿用[同日完整自动检查](../checks.json)，没有重建或重复无关测试。

本机 Windows 11 x64，Intel UHD Graphics 630，两块 DELL D2721H，各 1920 × 1080 / 96 DPI。主屏 `(0,0)`、副屏 `(1920,42)`，稳定工作区高度 1032。普通权限正常模式使用独立实例 `rs06-final-femtovg`、独立数据目录及 `Ctrl+Alt+K`，PID 194856、HWND `DF0AC4`。本轮没有修改电源或显示设置，没有注入拔插、几何、托盘或恢复消息。

## 实际结果

| 场景 | 结果 | 证据 |
| --- | --- | --- |
| 拔出前基线 | 副屏 `(2560,248)`，640 × 264；实际输入 `code` 得两项结果，观察到 FemtoVG `NativeOpenGL` 完成帧 | [截图](screenshots/femtovg-before-unplug.jpg)、原生快照 |
| 实际断开视频线 | 用户确认拔下并保持断开，探针只枚举主屏；原 HWND/PID 保留，窗口回到主屏 `(640,206)`，全部位于工作区内，托盘仍注册 | [人工确认](human-disconnect.json)、[原生快照](native-observations.json) |
| 断开后输入与热键 | 追加 `x` 成为 `codex`，回调同时为组件焦点/原生焦点/可见 true，内容正常重绘；从独立临时前台窗口实际按 `Ctrl+Alt+K`，查询和原窗口保留 | [输入](screenshots/femtovg-physical-disconnect-input.jpg)、[热键呼出](screenshots/femtovg-disconnected-hotkey.jpg) |
| 实际插回视频线 | 用户确认双屏恢复，探针重新枚举原主副屏及 96 DPI；原 HWND/PID、查询和托盘保留。Windows 将窗口恢复到原副屏可见位置，应用最终按稳定任务栏工作区约束 | [人工确认](human-reconnect.json)、原生快照 |
| 重新连接后热键与输入 | 实际热键呼出仍显示 `codex`；追加 `x` 成为 `codexx`，实际输入及非空白内容更新正常。整个会话只创建一次原生窗口 | [呼出](screenshots/femtovg-reconnected-hotkey.jpg)、[输入](screenshots/femtovg-reconnected-input.jpg) |
| 正常退出与清理 | 点击底栏“退出”，日志为 `lifecycle_quit`、`desktop_stopped tray_removed=true`；临时前台夹具通过标题栏关闭，最终无进程、窗口、端点，测试热键重新取得并释放成功 | [cleanup.json](cleanup.json) |

84 条测量事件全部 `dropped=0`，最终日志原文及 SHA-256 见 [desktop-events.json](desktop-events.json)。原生探针始终无参数只读，快照、探针哈希见 [native-observations.json](native-observations.json)。5 张已检查的截图及指纹见 [screenshots.json](screenshots.json)，自动核对结果见 [result.json](result.json)。本机原始文件保留在忽略目录 `.reasonix/rs06-femtovg-unplug-20261103/`。

最终双屏原排列及 100% 缩放保持，原电源值仍为交流 1 / 直流 0。没有重采内存、延迟或其他性能指标，不将此轮交互通过解释成渲染器性能选择。
