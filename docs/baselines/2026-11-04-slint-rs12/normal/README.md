# RS-12 普通模式可见状态补采

2026-11-04（北京时间）。前一轮 FemtoVG 普通实例 PID 284000 已在收尾时清理，用户稍后回复“未看到窗口”对应的是旧提示。随后重新复制相同 seed，先核对新进程与所属原生窗口，再由用户实际 Alt+Space 呼出、等待扫描结束、滚动首页到底/回顶、搜索 `code` 与“计算器”并清空后确认预热。两种渲染器顺序运行，采样期间无构建或其他基准。

使用[本轮冻结的相同 exe](../build-identities.json)，实例 ID 和数据目录分别隔离。程序未设置 `--measurement-log`、`--measurement-business` 或 `--measurement-stdio`，既无测量日志线程，也无匿名管道控制线程；原生观察按实际 PID/窗口类定位 HWND。扫描/预热由用户确认，报告保留 `OperatorBusinessConfirmed=true`，内部扫描警告、目录数量和应用缓存字段不可观测，继续留空。

| 可见空闲中位数 | Software | FemtoVG |
| --- | ---: | ---: |
| PrivateBytes（MiB） | 26.32 | 55.03 |
| 私有工作集（MiB） | 21.85 | 46.93 |
| WorkingSet（MiB） | 63.97 | 124.09 |
| CPU（全机 %） | 0.013000 | 0.058144 |
| 有效样本 | 3 × 15 秒 | 3 × 15 秒 |

两组全部有效，模式均为 `Production`、单进程、窗口 DPI=96，逐次原始字节、范围、时长、GPU、线程/句柄及采集器指纹见 [Software](software-visible.json)、[FemtoVG](femtovg-visible.json)和[汇总](resources-summary.json)。PrivateBytes 是私有提交，WorkingSet 含共享页，不能混用；CPU 按机器逻辑处理器数归一化。

扫描后各 736 个缓存条目的 ID、名称、路径、目标、参数、工作目录和来源与初轮一致；配置、历史和应用库原文哈希亦一致，见[数据核对](data-comparison.json)。本轮是普通模式的可见补采，不与前一轮探针结果合并求中位数；实际预热路径和采样时点不同，不能把两轮差额归因于探针开销，也不把可见结果推广到隐藏或默认网络设置。

用户的实际预热确认见 [Software](software-operator.json)、[FemtoVG](femtovg-operator.json)。两份程序均由用户正常退出，退出码 0、无强制清理，见 [Software 退出](software-completion.json)、[FemtoVG 退出](femtovg-completion.json)。最终无 Velo 进程或测试端点，Alt+Space 可重新注册，正式目录原始文件哈希未变，见[清理](cleanup.json)。

本次新增 **6 个有效可见空闲样本**；该检查点加上原轮 36 个共 42 个。随后[隐藏态与实际输入续验](../remaining/README.md)又新增 12 个有效隐藏样本，累计 54 个；两种 Slint 的默认网络行为和实际输入边界已有记录。校准暖启动/热键性能、Wails 人工输入、独立冷条件和 DPI/输入到呈现等专项矩阵仍待验，默认渲染器未决定。
