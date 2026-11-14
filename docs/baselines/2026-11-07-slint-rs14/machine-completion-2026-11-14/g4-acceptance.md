# G4 当前机器验收结论

2026-11-15（北京时间）。**RS-14 已完成，G4 在用户指定的当前机器范围放行。** 范围为现有 Windows 11 x64 / Intel UHD 630 宿主机及其 Windows 10 Pro 19045 / VMware SVGA 3D 测试客户机。其他实体 GPU/驱动和本机不可选的 200% DPI 按用户要求跳过，不扩大兼容承诺。

Software 继续作为已测范围的默认 renderer，FemtoVG 保留显式替代构建。两条路径各启用唯一 renderer；[当前程序](product-builds.json)各 221 项输入绑定完整检查、85 项 Rust 测试及组件冒烟。固定 Go 1.25.6 / Vue 回归、前端 26 项测试和两 renderer 的 Go/Rust 双向数据差分通过，见[验证汇总](verification-summary.json)。

[系统验收](machine-acceptance.json)的 4 阶段、8 项断言及 3 次普通权限程序会话通过：官方 Go v0.9.11 原位接管、锁定和未知版本拒绝、替换后故障恢复、renderer 系统升级、Go 精确回退、Rust 卸载保留未知文件与恢复档案。正式数据层打开前完成每用户原文快照；系统安装器未以管理员身份改写用户数据。

[搜狗实测](sogou-acceptance.json)补齐同两份当前 exe 的人工键盘验收。用户分别确认候选定位、翻页、方向键/空格选词、Esc 只取消候选、Enter 仅提交及中英切换正常，随后从托盘退出。截图保留真实候选框；日志记录 Software/FemtoVG 各 74/69 次原生按键、2/3 组 winit 组合/提交事件，双方无误启动、无组合期间隐藏、无事件丢失，退出码均为实际观察到的 0。配置、历史、应用库原文字节未变。初次候选截图时 winit 组合事件尚未出现，该观察与后续实际事件分别保留。

RS-12 资源/延迟与 RS-13 视觉、微软拼音和 DPI 的原证据保留其原 exe 身份，不宣称在当前安装器构建重采了性能或所有 DPI。FemtoVG 常驻内存超预算、Software 动画帧节奏取舍、10000 项慢查询及历史失焦原因缺证仍明确保留；本次放行不把未达成项改写为达成。

前轮真实 GitHub CI 成功记录仍绑定提交 `4bab4c940fd01c7a1219cc4991ddeaadf5a37e26`。当前正式候选的全部本地 workflow 命令已验证，YAML 已检查；新增 product workflow 阶段尚未在远端执行，不冒称当前源码获得了新 CI success。按本次仅考虑现有机器的范围，不为重复远端运行新增 runner 或推送。

[末尾清理](sogou-final-cleanup.json)、[热键释放](sogou-hotkey-cleanup.json)与[正常关机](sogou-guest-shutdown.json)通过。客户机测试实例、端点、注册、快捷方式和提权计划任务均无遗留；原始合成数据、程序回退档案和日志保留，宿主机正式数据未用于测试。

[宿主机最终核对](sogou-host-cleanup.json)确认正式文件仍匹配本轮参考、无 Velo/采集测试进程、原工作分支与空暂存区保持；测试虚拟机未运行。

本结论是当前机器的技术验收。内部版本 `0.1.0`、`RedistributionCleared=false`、原 `release-policy.json` 和已打包产物中的 `G4Passed=false` 保留打包时原值及哈希；这份后续验收绑定精确 exe 和安装器，不倒改旧包。未发布 Release、未替换宿主机正式安装。公开分发、版本推进和发布仍是独立的后续动作。
