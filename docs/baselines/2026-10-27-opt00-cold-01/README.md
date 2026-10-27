# OPT-00 首次冷启动实测：输入就绪超时

公开说明：本页关联的公开结果已脱敏；原始日志与源码快照仅本地保留，见[公开范围与校验口径](../README.md)。

2026-10-27 已完成一次符合重启后首次运行条件的冷启动观察。**结果为失败样本：约 15 秒内未确认搜索框可输入，用户也反馈“不能直接输入字母”。**有效冷样本为 0 / 3，中位数与 P95 保持空缺，不将超时填成成功耗时。

| 事件 | 北京时间 / 结果 |
| --- | --- |
| 重启前冻结准备 | 21:56:06 |
| 本次系统启动 | 21:59:11.500 |
| 冷条件检查 | 无 Velo 进程；实际登录启动项已暂时移除 |
| 测试进程创建 | 22:04:55.8739363，PID 31144 |
| 输入就绪等待超时 | 22:05:11.1488621，`Valid=false` |
| 测试版正常退出 | 22:05:12.959564 |
| 安装版后台恢复 | 22:05:13.969427，PID 26904 |
| 原登录启动项恢复 | 22:05:14.7917975，系统注册表服务核验通过 |

## 原始证据与统计

- 原始事件（本地原件：`coldstartup-gui-20261027-140235-092-events.jsonl`，不入库）、[会话状态](coldstartup-gui-20261027-140235-092-state.json)、[程序 / 输入 / 环境上下文](coldstartup-gui-20261027-140235-092-context.json)。
- [冷条件预检](cold-preflight.json)、[OS 进程创建及观察时钟](process-clock.json)、[用户反馈](user-observation.json)、[逐项证据复核](evidence-verification.json)。
- [本次 CSV](desktop-latency.csv)与[最终汇总](cold-summary.json)：1 次失败、0 次有效，分位数为 null；`DurationMilliseconds=0` 是旧探针超时记录的占位值，**不是 0 ms 启动**。
- 生命周期日志（本地原件：`application-lifecycle.jsonl`，不入库）显示页面 `frontend interactive=427 ms`、窗口可见、扫描完成可见应用 389 项；这些状态不等同于搜索框可输入，不能替代端到端终点。

OS 进程创建到超时记录的观察窗口为 **15274.9258 ms**，只描述观察截止时间，不是一次成功启动的耗时。超时原始记录中的空 DOM / 原生字段是未收到输入就绪报告时的默认值，不能据此断言窗口隐藏；会话 `Status.visible=true` 与日志也明确记录窗口可见。

冻结 v2 的 SHA-256 仍为 `D099F92BF3A71A804572A2F0CF81B5FFEA664D46E0F1E477B7C37C7183E755C6`，测量后校验通过。环境仍为 Windows 26300、WebView2 155.0.4283.62；本次启动的初始五份输入指纹与此前热呼出 / 暖启动上下文一致。启动期间正常扫描将可见项更新到 389；下一轮输入另行冻结，不能据此宣称扫描后数据完全未变。

旧冻结二进制的超时事件缺少程序 SHA 与 BootID，导出时程序身份取自该会话构建上下文，BootID 仅在进程号及 Kind 都匹配时补入，并标记 `BootIdSource=MatchingSessionContext`。原始 JSONL 未改写。新增回归同时验证不能借用其他进程的 BootID，见导出回归（本地原件：`collector-regression.txt`，不入库）。

## 运行状态恢复

[结束记录](completion.json)确认测量前后的用户输入文件指纹一致，随后恢复了原安装版后台实例。[恢复记录](startup-restore.json)与[最终系统状态](startup-status-final.json)确认原登录启动值已经恢复，StartupApproved 仍启用。私有 `cold-cycle.json` 已更新为 `StartupRestored` / `RestoreRequired=false`，没有遗留临时停用状态。

本轮未修改产品焦点逻辑。结合此前三次暖启动及用户反馈，首次启动直接输入仍是未解决的回归问题；应先修复并用新程序身份重新测量，再补齐有效的冷 / 暖启动分布，而不是继续重启收集相同失败。

此前的[热呼出与暖启动阶段](../2026-10-27-opt00-latency/README.md)及其 63 文件校验清单保持不变。合并查看本轮全部指标可用[累计 CSV](desktop-latency-all.csv)及[累计汇总](desktop-latency-all-summary.json)；本次冷失败不参与分位数。

采集源码快照与本阶段全部文件由 `SHA256SUMS.txt` 固化。OPT-00 继续“待验证”，未宣称启动 / 热呼出性能目标全部达标。
