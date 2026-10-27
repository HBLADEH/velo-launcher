# OPT-00 启动与热呼出计时采集

公开说明：本页关联的公开结果已脱敏；原始日志与源码快照仅本地保留，见[公开范围与校验口径](../README.md)。

本轮已取得 **20 次正式热呼出的有效原始记录**，另有 1 次真实按键校准。三次正式暖启动已观察到首次输入焦点异常，尚不能形成正常自动聚焦的启动分布；真正冷启动仍需独立重启。所有延迟均为保守上界，不能据此宣称全部达到 500 / 50 ms 目标。既有资源矩阵及其校验清单保持不变。

## 当前原始记录

| 分组 | 有效 / 要求 | 已保存的无效尝试 | 当前结论 |
| --- | --- | --- | --- |
| 正式暖启动 | 1 / 3 | 2 次等待输入就绪超时 | 仅一次终点确认，12904.2182 ms；首次输入焦点异常 |
| 真正冷启动 | 0 / 3 | 无；当前系统启动早于探针准备 | 需要独立重启后的首次启动 |
| 正式热呼出 | 20 / 20 | 3 次等待超时、1 次原生前台未确认 | 中位数 29.4425 ms，P95 51.9961 ms |

正式采样为同一冻结 v2、同一输入 / 环境指纹下的 24 次尝试，20 次有效。全部有效样本均为真实按键（`InjectedKeyboardEvent=false`），搜索框文档 / 输入焦点、可编辑与显示条件成立，原生窗口可见且前台 PID 匹配。最小 / 最大值为 27.5048 / 55.4893 ms，P95 使用最近秩。4 次失败也保留在[正式采样 CSV](hotkey-formal-utc.csv)；[正式汇总](hotkey-formal-utc-summary.json)可直接复算，未删除有效慢样本。

正式采样前，用户确认一次真实快捷键呼出后成功输入字母；该校准上界 58.7829 ms，原生前台与 DOM 条件均通过，单独保留为 `physical-calibration-01`。将它与正式样本合并时共有 21 次有效记录，中位数 29.5415 ms、P95 55.4893 ms。正式分布与含校准分布同时披露。当前 P95 上界超过 50 ms，尚不能证明原目标达标，也不能从上界反推真实延迟必然超过 50 ms。

另外保留 3 次无效自动按键试验：v1 / v2 校准上界 70.1545 / 55.6196 ms，以及 `automated-01` 的 45.7324 ms。三者均报告原生前台 PID 为 0、带注入标记，不能计入有效基线。系统会话检查显示 active / unlocked，不能据此认定锁屏或产品焦点缺陷。

[逐条 CSV](desktop-latency.csv)保留失败原因、注入标记和时钟误差；[热呼出阶段完整 CSV](desktop-latency-hotkeys.csv)保留当时全部事件。只有尚未取得的指标保持 null。同一事件在原始会话和校准副本中出现时只统计一次，`Evidence` 保留全部来源。历史记录缺失环境 / 输入证据时明确标为 unverified，不能改用最新构建的身份。重采集后生成新的汇总文件，旧汇总按其 InputSHA256 对应当时输入。

## 暖启动与首次输入焦点

[三次正式暖启动](warm-formal-utc.csv)及其[汇总](warm-formal-utc-summary.json)均已保存。PID 5444 在 12904.2182 ms 时确认了可输入终点；PID 6344、9012 超时。只有一次终点确认，不能据此报告有代表性的暖启动中位数 / P95。更早的 v1 / v2 自动启动各有一次超时，也保留在完整事件集中。

用户随后复核并明确反馈：“第一次不可以，点击搜索框之后后面的都可以了”，见[用户观察](user-focus-observation.json)。额外的 PID 11156 手动复核在 7862.2436 ms 时确认终点，见原始事件（本地原件：`manual-focus-check-events.jsonl`，不入库）和[上下文](manual-focus-check-context.json)；已按 `ManualFocusRecovery` 方法单独分组，不能混入自动启动性能。这些数值包括等待 / 人工恢复焦点，不能当作通常的加载时间。日志中约 407–421 ms 的 `frontend interactive` 也不能替代输入就绪。

额外复核期间，第二实例在进程互斥检查前覆盖了共享 `control.json`，指向已退出的 PID 18412。原始追加事件已先归档；仅核对路径及启动时间后关闭测试 PID 11156，见[清理记录](manual-focus-check-cleanup.json)。采集器 `Finish` 现会解除旧会话布防，同时保留独立 `LOCALAPPDATA`，避免采集结束后的再次手动启动污染旧会话。正式采集时不要在前一实例仍运行时重复打开快捷方式。

[阶段完整快照](desktop-latency-final.csv)与[阶段汇总](desktop-latency-final-summary.json)固定保存本轮结果。OPT-00 继续待验证；本轮未修改产品焦点逻辑，也没有将启动失败隐藏为成功样本。

## 测量边界与构建身份

测试版使用正常生产界面、独立数据副本和 `latencyprobe` 编译标签。普通构建排除 Go 探针与前端观察代码；未修改 `app.go`、`main.go` 或前端产品源文件。

- 启动：OS 进程创建时间 → 实际搜索框聚焦、可编辑、显示且原生窗口可见 / 前台时的 Go 接收时刻。
- 热呼出：低级键盘钩子观察到 Alt+Space → 同一输入就绪条件的 Go 接收时刻。钩子只保留已布防的快捷键，不保存输入文字。
- 结果是包含观察 / IPC 开销的保守上界；不能减去这些开销来声称性能达标。v2 快捷键时钟不确定度为 16.625 ms，页面有 25 ms 的兜底检查，焦点 / DOM 变化也会触发观察。
- 页面焦点成立而原生前台未确认的事件仍判无效；超时另存失败。`frontend interactive` 日志和工具调用耗时不代替端到端边界。

当前冻结 v2：`build/bin/velo-launcher-latency.exe`，14,209,024 字节，SHA-256 `D099F92BF3A71A804572A2F0CF81B5FFEA664D46E0F1E477B7C37C7183E755C6`。产品代码基于 `45dffab58c2fa17993be19da8f5b76f00f6a4575`，另含未提交测量代码。构建身份见 [v2 清单](build-and-input-v2.json)，冻结源码见 Go 快照（本地原件：`probe-source-v2.go.txt`，不入库） / JS 快照（本地原件：`probe-client-v2.js.txt`，不入库）。当前 Go 工作区在冻结版基础上给超时记录补了二进制与 BootID 字段；已有记录仍按各自实际构建归档。

v1 SHA-256 为 `31D38E9092717B6119FABBD079A170AA4605C47CA773E1C67C884F58D2A2CB40`，不得与 v2 混合。机器环境延续 Windows 11 26300 / WebView2 155.0.4283.62 / DPI 120；各新会话还会保存 Windows、CPU、实际 WebView2 版本及五份输入文件指纹。只有输入身份一致的记录才会合并；热呼出前后输入变化时排除该会话记录。

## 在正常交互桌面补采

工作目录已准备为 `.reasonix/opt00-latency-20261027-1240`，快捷方式为其下 `launch/Velo timing.lnk`。工作目录内的控制令牌仅供本机采集使用，不应复制到公开证据中。

真实按键校准和正式热呼出已完成。重新进行整轮采集时，可在本机普通 PowerShell 7.4+ 终端、仓库根目录运行：

```powershell
./scripts/run-desktop-latency.ps1 `
  -WorkDirectory .reasonix/opt00-latency-20261027-1240 `
  -OutputDirectory docs/baselines/2026-10-27-opt00-latency
```

脚本正常退出 / 重启三次取得暖启动记录，然后逐次提示按 Alt+Space。每次隐藏后等待至少两秒，只计入至少 20 条确认有效的真实按键；请勿按 Enter 启动候选。扫描完成后才开始热呼出。无效尝试不会删除。自动化进程环境中的前台观察曾失败，因此实际采样需要在用户正常交互桌面核验。

也可用 `run-desktop-latency.ps1 -HotkeysOnly` 在已运行的探针上单独采集按键。暖启动通过 `collect-gui-startups.ps1` 布防三次，用户从同一快捷方式启动，采集器完成后正常退出每次进程。一次采集器相对路径错误导致快捷方式启动即退出，已记录在 collector-failures.jsonl（本地原件：`collector-failures.jsonl`，不入库）；修复后侧车 / 控制目录均转换为绝对路径，并增加不启动真实应用的相对路径回归。

脚本结束时正常关闭测试版、检查真实用户输入文件，并冻结下一次冷启动的输入 SHA-256、文件元数据和准备时间。当前准备清单原本未捕获运行中的安装版，`Restore=null`，不能宣称本轮已恢复一个原本不存在的进程。

真正冷启动每次需要用户自行保存工作并重启，且该次系统启动后还没有运行过 Velo 或探针。`Finish` 已完成，下一次输入身份见[next-cold-input.json](next-cold-input.json)；当前系统启动早于准备时间，[同次启动条件拒绝检查](cold-condition-check.json)通过，未误标为冷启动。重启后不要提前读取 / 校验探针程序或测试数据，也不要重新构建。然后运行：

```powershell
./scripts/run-desktop-latency.ps1 -Cold `
  -WorkDirectory .reasonix/opt00-latency-20261027-1240 `
  -OutputDirectory docs/baselines/2026-10-27-opt00-latency
```

按提示通过资源管理器打开准备好的快捷方式，等搜索框出现并保持前台，再回终端采集。`Arm` 只检查冻结程序 / 数据的文件元数据与启动条件，避免测量前读取文件内容；`Capture` 测量后再校验程序 SHA-256。旧的 `-Action Cold` 已拒绝执行。每个 BootID 只允许一次冷尝试，失败也保留；需三个不同重启条件下的有效样本。脚本不会重启电脑或更改自启动设置。

当前已发现首次启动焦点问题。继续测量此版本时，不额外点击搜索框恢复焦点；无法直接输入即记录失败。若先修复产品焦点，应另建程序身份及新一轮计时记录，不能覆盖本次失败基线。

## 工具验证

- `go test -tags latencyprobe ./...` 通过，包含事件时钟回绕与下界计算。
- PowerShell 脚本解析及最终采集导出回归（本地原件：`collector-regression-final.txt`，不入库）通过：重复证据去重、来源保留、输入指纹、输入变化排除、冲突副本拒绝、旧冷启动路径拒绝、相对调用路径转绝对目录、重启后 PID 复用隔离与保留测试数据目录的解除布防。
- 完整验证日志（本地原件：`verification-final.txt`，不入库）：`verify.ps1 -SkipBuild` 通过，含前端 25 项测试、lint / 类型检查 / 生产构建、Wails 绑定、Go vet / 默认全部测试。

这些结果只验证测量工具与普通构建回归，不代表已完成桌面计时验收。

最终 CSV 保留 ISO 8601 UTC 小数秒与进程 ID，早期派生 CSV 继续作为历史快照保留。正式数据已逐条对应原始 JSONL 校验，见[evidence-verification.json](evidence-verification.json)。当前采集器源码快照见collect-latency-source.ps1.txt（本地原件：`collect-latency-source.ps1.txt`，不入库），本阶段全部文件校验见SHA256SUMS-phase1.txt（本地原件：`SHA256SUMS-phase1.txt`，不入库）。
