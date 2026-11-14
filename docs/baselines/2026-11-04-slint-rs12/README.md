# RS-12 同轮业务性能矩阵

2026-11-04（北京时间）开始执行，前置 [G3 本机验收](../2026-11-04-slint-rs11/g3-acceptance.md)已放行。2026-11-11 **RS-12 本机测量矩阵已验证**，见[综合验收](completion-2026-11-11/README.md)。默认渲染器未决，预算差异与历史失败保留，不把合成操作或 RS-04 原型数值当作完整产品/发行验收。

2026-11-11 最新[独立冷启动机器计时](completion-2026-11-10/calibrated-cold/README.md)已完成 **9/9**，各候选 3/3，最后 FemtoVG 上界 473.743 ms；Software/FemtoVG 六份均在 500 ms 内，Wails 原三份未验证该预算。原程序/工具/seed、不同 boot、来源及完整日志逐项复核通过，正常退出与清理完成，无需为本矩阵再重启。九份人工冷字符观察保持原范围，下文各历史检查点不改标；历史失焦原因未知、FemtoVG 内存超预算等限制进入最终验收，默认 renderer/G4 未定。

2026-11-10 最新[暂离自动补验](completion-2026-11-10/remaining-auto/README.md)：三候选各3次暖启动/20次机器热键接收校准完成，最终热键上界P95均在50ms内，物理按键记录独立保留。六条件30次查询/动画整树CPU及桌面帧节奏已测，最终共享入口和资源清理通过；新增工具焦点拒绝原件保留，历史人工失焦原因未被反推。冷字符9/9不变，新机器冷计时0/9，最终九份副本已准备，需未来各自独立重启，未自动重启或更改登录设置。RS-12继续进行中。

2026-11-10 [三候选 × 1000/10000 项查询帧补验](completion-2026-11-10/frames/corrected/matrix-final/README.md)：使用第四版实际图标夹具与修正观察器，六组共 **33 次固定物理 A 查询**，另两条编辑保留；结果参考、几何/时钟、正常退出、末尾日志和资源清理通过，当前无测试实例残留。旧回退图标、几何/墙钟与空采集失败保持原样，未混入新矩阵。冷字符 9/9 不变；精确首次可输入、目标动画 CPU/GPU 和历史问题归因仍待验，默认 renderer/G4 未定。

2026-11-10 最新[独立冷条件补验](completion-2026-11-10/README.md)：最后一份替补 `software-5` 正常退出，实际非合成字符和 exe 身份核对通过。九个不同 boot ID、原件/冻结身份及完整矩阵复核通过，累计 **9/9**（三种程序各 3/3），原两次 Software 无效尝试保留。171223.533 ms 上界含人工等待，不验收首次可输入或冷启动速度。正式索引/历史与下午参考不同，最后写入时间均早于本次重启，没有本轮启动前正式哈希，不作变化归因。实例/窗口/热键清理通过；独立冷条件字符观察不再缺样本，精确延迟、帧呈现和既有失败归因继续待验，RS-12 未放行。

2026-11-09 此前[独立冷条件续验](completion-2026-11-09/README.md)：替补 `software-4` 取得有效非合成编辑记录，正常退出且 exe 身份一致。该次累计 **8/9**（Software 2/3、FemtoVG 3/3、Wails 3/3），原两次 Software 无效尝试保留。两条一字节编辑中，首条合成状态被排除，第二条非合成状态作为终点；约 36 分 35 秒上界含人工等待，不验收冷启动速度。正式索引/历史与下午参考不同，文件最后写入时间均早于本次重启，没有本轮启动前正式哈希，比较范围如实保留。原记录保留，`software-5` 已按上文完成。

此前 `femtovg-3` 将累计推进到 7/9；用户确认先中文后英文并再次热键呼出，五条编辑回调完整保留，采集器选中较早的三字节非合成回调。约 2.416 秒上界含人工操作，不验收冷启动速度；原记录保留。

此前 `software-3` 将累计推进到 6/9；首次显示后因失焦隐藏，热键再次呼出才接收字符，约 30 秒上界含人工等待与再次激活。原记录不证明首次显示即可输入或冷启动速度，继续保留。

此前 `wails-3` 取得第三份 Wails 独立周期有效字符观察，正常退出且 exe 身份一致，将累计推进到 5/9；约 25 秒上界含人工等待，不验收冷启动速度，原记录保留。

2026-11-09 [Wails 热键续验](completion-2026-11-09/wails-hotkey/README.md)：新会话取得 20 个物理起点/20 条可信、聚焦、非合成字符，零配对失败和事件丢失，用户报告全部正常；正常退出、正式文件未变及资源清理已核对。旧采集器退出检测失败和辅助窗口报告丢失如实保留，已修正并通过共享完整验证。单轮样本数补足，此前间歇性失败未归因，总体稳定性与精确延迟继续待验。冷条件仍为 4/9，下一份为 `wails-3`。

2026-11-09 [独立冷条件续验](completion-2026-11-09/README.md)：预冻结 `femtovg-1`、`wails-1`、`femtovg-2` 和 `wails-2` 各取得一条实际聚焦字符接收记录并正常退出，独立冷观察为 **4/9**（Software 0/3、FemtoVG 2/3、Wails 2/3）。计时均包含人工等待，部分轮次还有字符前窗口拖动，不能判断冷启动预算；`software-1` 和 `software-2` 无效尝试保留，后者的唯一字符事件标记 `composing=true`，已准备替补 `software-4` 和 `software-5`。旧采集器的环境 ID 因字段顺序不稳定而产生差异，已修正后续采集并保留原报告。下一项为 `wails-3`，需另一次独立重启。

2026-11-05 [续验](followup-2026-11-05/README.md)追加启动扫描树观察与 Wails 人工输入问题：虽然匹配到 20 次实际字符，用户报告闪现隐藏/不能输入；五次起点诊断与人工轮次未完全对齐，稳定性不放行。修正采集器持续至退出、保留重试和失败的规则，原始样本与人工反馈均保留。随后三份 Wails 暖启动采样进程正常退出、最终实际字符核对通过，上界中位数 1551.168 ms、范围 1365.629–8741.970 ms，含人工等待；第一份另一次启动单列，不混入三次统计。严格冷条件、校准延迟和帧呈现仍待验。

同日中午 [Wails 对照续验](followup-2026-11-05/wails-retry/README.md)取得单次人工呼出/可输入反馈与实际字符记录，但桌面工具未返回窗口且随后停止，没有新增 DPI/帧呈现样本。三份缺少真实预热依据、模式分类错误的新资源记录全部排除，修正显式空闲依据及 Go 探针分类；原 54 份有效样本不变，原 Wails 热键失败不撤销。

用户随后明确安排进入严格冷启动，九份原冻结实际输入副本预检通过，首份 `software-1` 待下一次独立重启后启动，见[最终补验检查点](final-checkpoint/README.md)。当前周期已运行程序，冷样本仍为 0/9；没有自动重启或修改登录项，其余未验项目不因预检通过而放行。

## 程序与数据

2026-11-07 用户安排先补齐冷启动和物理 IME，恢复冷条件采集准备；目前缺少录像/校准条件，精确延迟和帧呈现暂留待验。Software、FemtoVG 和 Wails 的物理微软拼音四项均已通过本机 96 DPI 范围的人工/日志核对，三份实例正常退出；九个独立冷周期按[最新补验记录](completion-2026-11-07/README.md)推进。原始样本、热键稳定性失败、冷条件 0/9 和 renderer 未决状态均保持真实范围。

- Wails 使用官方 v0.9.11 CI Release，SHA-256 `9A1A898A1E3D70267481BCC68E3368F0B6594F68E8AE43B3F0DCDC06A9B329A6`，对应发布提交 `abde1353117add4b58b9c34eb442e1ad9c445d97`；不是当前脏工作区重建的 Go 程序。
- 两种 Slint 使用锁定 Rust 1.99.1 / Slint 1.18.1、同一源码输入、winit 后端、Release 优化，只改变渲染 feature。具体 exe 与源码指纹随本轮证据保存。
- 业务数据从旧数据复制到隔离目录，统一浅色/列表、Alt+Space、关闭更新网络检查/登录启动、刷新间隔 1440 分钟；保留正常后台发现。每轮重新复制同一 seed，不复用上轮已修改的数据。默认网络设置回归单列。
- 本地原件在 `.reasonix/`：冻结 exe/依赖清单、tracked 脏补丁及包含 untracked 迁移模块的源码副本、完整输入、原始资源/事件和检查日志；不提交用户路径、应用列表、原始查询、程序和用户文件。

## 采集改动

原 RS-04 资源采集器将所有 Slint 进程记录为固定 28 项模型，不适用于当前业务程序。本轮显式区分 `Fixtures`/`Business`，业务样本保存实际 JSON 与磁盘 PNG 指纹、目录数量、扫描状态和应用图标缓存；事件丢失、扫描中或采样期间数据/状态变化时拒绝空闲验收。默认拒绝扫描警告；本机部分目录/图标不可读时，显式的 `AllowScanWarnings` 例外还要求扫描期间发布了新快照，警告及适用范围保留在报告中，不能将失败后只保留旧快照当作完成发现。

`--measurement-business` 保留正常实例锁、热键、托盘、业务后台任务及显隐回调，必须指定测试 ID/数据目录。匿名管道中的固定搜索/页面命令只提供合成负载，字符事件标记 `synthetic=true`，不计为真实输入。未开启测量时不创建控制或日志线程；CPU/纹理估算、磁盘图标、GPU 计数分别报告。

## 本轮检查与结果

本目录保存已完成范围的整理结果，未覆盖项继续保留待验。首次构建中的基准工具时间字段反序列化错误已保留本地失败日志，并改为解析 RFC3339 字符串，未为此增加 chrono features 或依赖。首轮业务短测发现 `.NET File.ReadLines` 的共享模式拒绝 Rust 正在写入的日志；改为显式 `FileShare.ReadWrite | Delete`，只解析已换行的完整记录，并用保持写句柄打开的回归测试验证。原失败进程在清理等待后强制结束，后续两种短测均正常退出。

两种当前构建的 clippy、各 80 项 Rust 测试、双向数据兼容、原生集成及窗口/固定/业务组件冒烟已通过。FemtoVG 的完整页面入口通过；Software 首轮在失焦隐藏后网格按键断言失败，第二次在失焦后等待超时，两份原日志保留；用户协助保持前台后，同一 exe 的全新目录完成设置、网格、导入、备份与重启两轮检查，正常退出、托盘清理通过。没有通过修改产品失焦规则或忽略断言使测试变绿。

| 项目 | 当前状态 |
| --- | --- |
| 两种当前 Slint 完整检查与 Release | 两种各 80 项测试及相关入口通过；Software 页面两次前台失败与人工协助复验分开保留 |
| 旧版 Go/Vue 与共享工具回归 | 初轮与本次续验 `verify.ps1 -SkipBuild` 均退出 0；实际输入 `latencyprobe` 全部 Go 回归通过，新图标 Rust example 两种 feature clippy/Software Release 通过 |
| 1000/10000 项 Go/Rust 内核、完整查询 | 已采，144 个计时记录有序结果一致；1000 项完整查询满足本合成范围的 <10 ms 目标，见下表 |
| 三份业务版可见、隐藏 30 秒/5 分钟 | 三份各三态 3 × 15 秒有效；Wails 首次五分钟组的显隐变化失败保留，同一进程重采通过 |
| 100 次显隐、30 分钟操作/空闲 | 两种 Slint 的 100 次合成显隐/30 分钟循环已完成；Wails 用户确认 100 轮真实显隐及页面操作，30 个一分钟样本累计 1815.35 秒，只读观察完成并正常退出 |
| 暖启动各至少 3 次、真实热键各至少 20 次 | 三份候选各 3 次暖启动及各至少 20 次物理热键字符观察已记录，含人工等待；Wails [2026-11-09 新会话](completion-2026-11-09/wails-hotkey/README.md)有 20/20 配对、用户报告正常，原失败保留。另以最终 build8 完成各 3 次暖启动/20 次机器热键接收校准，热键上界 P95 均 <50 ms，见[自动补验](completion-2026-11-10/remaining-auto/README.md)；机器与物理记录分开，历史人工失焦尚未归因 |
| 严格冷启动各 3 个独立冷条件 | 人工冷字符观察 [9/9](completion-2026-11-10/cold-matrix.json)与机器冷计时 [9/9](completion-2026-11-10/calibrated-cold/README.md)分别保存。机器记录各候选 3/3、九个不同 boot，Software/FemtoVG 上界 P95 为 287.492/473.743 ms、六份均在 500 ms 内；Wails 三份未验证该预算，全部原失败与等待保留，分位数仅作小样本观察 |
| DPI 转换、图标冷热、扫描/输入到结果呈现 | Go/Rust 系统图标离线对照、三候选启动/扫描完整树、有限实际 96/144/96 DPI 及本机 96 DPI 物理微软拼音已归档；无效轮次保留。第四版实际图标夹具完成[六条件 33 次物理查询帧](completion-2026-11-10/frames/corrected/matrix-final/README.md)，另完成[六条件 30 次查询动画 CPU/桌面帧采样](completion-2026-11-10/remaining-auto/README.md)。GPU 内部缓存/精确执行未直接测得，桌面变化帧不是无漏帧的呈现保证，不扩大本机测试范围 |
| Slint 无探针对照、默认网络设置 | 两种普通模式可见/隐藏 30 秒/五分钟均有有效记录，新增隐藏态 12 个后累计 54 个有效空闲样本；两种默认网络检查返回未提供 Rust 包，未安装；见[最新续验](remaining/README.md) |

## 搜索内核与完整查询

共享合成数据包含中文/英文长名称、4 个固定应用、固定系统工具，以及无历史/全部应用有历史两种状态；1000/10000 项、6 个查询、内核/服务、三轮各 256 次，共 144 个计时记录。先预热 8 次，使用固定时钟，磁盘写入/扫描/拼音归一化建索引不在计时内；内核预先计算历史权重，完整查询包含历史打分、应用/系统工具合并与固定标记。实际有序 ID 全部一致。Rust 核心不依赖 renderer，计时工具只用 Software feature 构建一次。

离线 Go 基准使用当前行为基线的私有固定时钟入口，源码指纹单列，不把测试工具当作官方发布 exe；桌面资源对照仍使用校验过的官方 v0.9.11 Release。

| 规模与层次 | Go 无历史范围 ms | Rust 无历史范围 ms | Go 全历史范围 ms | Rust 全历史范围 ms |
| --- | ---: | ---: | ---: | ---: |
| 1000 内核 | 0.024–0.317 | 0.019–1.608 | 0.028–0.312 | 0.074–1.332 |
| 1000 完整查询 | 0.027–0.312 | 0.018–1.556 | 0.118–0.412 | 0.196–1.523 |
| 10000 内核 | 0.275–4.118 | 0.190–14.462 | 0.349–4.006 | 0.903–14.594 |
| 10000 完整查询 | 0.281–3.939 | 0.188–14.510 | 1.420–6.361 | 2.748–16.652 |

表中是六查询 × 三轮的每次操作均值范围，不能称为逐次查询的 P95；原始纳秒和每查询三轮中位数见 [Go](search-go.json)、[Rust](search-rust.json)、[汇总](search-summary.json)与[计时身份](search-identity.json)。1000 项完整查询在本合成范围满足 <10 ms；10000 项扩展集的无匹配模糊查询最慢，Rust 带历史 `zzzz` 三轮中位数约 16.11 ms，保留结果而不宣称所有规模达标。当前 `matching` 的模糊分支逐条构造字符数组，是后续可剖析方向，本轮未在基线中途改搜索实现。

初轮 Go 与输入固化重叠，后续一轮 Rust 与页面协助重叠，均保留为本地开发记录、不进入此表。公开结果使用固化完成后独立运行的 Go，以及全部页面检查结束后独立运行的同一 Rust Release 工具；两边输入字节完全一致。工具构建不与计时重叠，结果文件保留各自原始哈希。

## 资源与循环

Software 首轮可见、隐藏至少 30 秒/5 分钟及 100 次合成显隐后的四组各 3 × 15 秒有效，PrivateBytes 中位数依次为 **24.09 / 24.09 / 24.09 / 24.11 MiB**。五分钟组分别在持续观察约 302/318/334 秒时开始；私有工作集约 19.81–19.86 MiB，工作集约 60 MiB，三者含义分别保留。Software GPU 提供程序未返回可读条目，保持 null。

Software 从预热后起完成 1800.08 秒的操作/空闲观察：100 次显隐后另有 26 次固定查询/设置/首页操作轮次，私有提交从 24.27 增至 25.39 MiB。末尾连续 12 个操作后观察点相同，跨度约 8.7 分钟；应用图标缓存由 34 增至 53 项，CPU/纹理估算从 1.06 增至 1.66 MiB。这支持本轮有限范围的预热后稳定观察，不能据此证明长期无泄漏。

结束后的可见复采前两个样本有效，第三个样本因失焦隐藏使整组无效，**不把该组汇总为三个有效样本**。该进程随后正常退出，原四组与完整循环记录仍有效；失败、原日志哈希和样本数量见 [失败记录](failures.json)。后续采集器对无效组保留原文件，并在同一进程中用新名称重采，最多三次，未关闭正常失焦隐藏规则。

初轮 Slint 资源轮次开启可选测量日志及匿名控制线程，报告模式为 `MeasurementProbe`；它们测的是完整业务与正常后台/生命周期，而不是完全无探针的常驻版本。后续普通模式可见补采单列于 [normal](normal/README.md)，不混入初轮统计，不能把观测开销视为零。

FemtoVG 的五组各 3 × 15 秒有效，PrivateBytes 中位数为 **53.52 / 53.52 / 56.88 / 53.66 / 54.18 MiB**，依次对应可见、隐藏 30 秒/五分钟、100 次显隐后可见、循环结束后可见；完成 100 次合成显隐、1802.26 秒操作/空闲观察，正常退出。操作后离散观察在约 65.72–69.18 MiB 之间，首末约 65.92/66.90 MiB；结束稳定可见组回落到约 54.18 MiB。不能仅用回落或本轮 30 分钟观察宣称长期无泄漏。完整字节、CPU、线程/句柄、GPU 和输入指纹见 [Slint 资源](slint-resources.json)、[循环观察](slint-loops.json)。

官方 Wails 的可见、隐藏 30 秒、隐藏五分钟重采的 PrivateBytes 中位数分别约 **173.04 / 171.26 / 163.58 MiB**，包含 7 个进程的完整树；此轮未开启 Wails `latencyprobe` 或诊断模式。用户实际预热首页/中英文查询，并确认 100 轮真实显隐和页面往返，见[操作记录](wails-operator.json)。三份扫描后缓存均为 736 项、可见目录为 619 项，排序后的缓存 ID 集合完全一致，配置及历史原文哈希一致，见[数据核对](data-comparison.json)。

Wails 完成了 30 个一分钟的操作/空闲样本，累计 **1815.35 秒**；30 个 CPU 样本均有效、进程树保持 7 个进程。端点私有提交首末为 170.09/167.45 MiB，全部轮询的范围约 161.63–172.88 MiB，详见 [Wails 循环](wails-loop.json)。这些样本不带 `IdleConfirmed`，包含操作者实际输入和显隐变化，不能改标成稳定空闲基线。

| 状态（PrivateBytes 中位数，MiB） | Wails 官方无探针 | Slint Software 业务探针 | Slint FemtoVG 业务探针 |
| --- | ---: | ---: | ---: |
| 可见空闲 | 173.04 | 24.09 | 53.52 |
| 隐藏至少 30 秒 | 171.26 | 24.09 | 53.52 |
| 隐藏至少五分钟 | 163.58 | 24.09 | 56.88 |

初轮三态各 3 × 15 秒，共 27 个有效样本；另有两个 Slint 显隐后及 FemtoVG 循环后的 9 个有效样本，初轮共 **36 个有效空闲样本**。无效组另保留，初轮汇总见 [资源数据](resources-summary.json)。后续两种无探针可见组新增 6 个、隐藏态新增 12 个，当前合计 **54 个**；可见[汇总](normal/resources-summary.json)与隐藏[汇总](remaining/hidden-resources.json)单列。GPU 不并入上表：Software 无可读实例；FemtoVG 可见首组 Dedicated=0、Shared 约 33.51 MiB、TotalCommitted 约 27.60 MiB；三者与进程 PrivateBytes 语义不同。

上述初轮对照含 Slint 观察线程与 Wails 无探针模式的差异，不据此直接发布收益百分比。后续普通模式可见中位数为 Software **26.32 MiB**、FemtoVG **55.03 MiB**；普通模式隐藏 30 秒/五分钟分别为 Software **26.12 / 26.12 MiB**、FemtoVG **67.26 / 67.30 MiB**。各轮时点和预热路径不同，不将差额解释为探针成本。Software 在已采范围内低于 50 MiB 可见/30 MiB 隐藏预算，FemtoVG 超出这两个预算。后续冷热启动/机器热键接收校准已按[最终验收](completion-2026-11-11/README.md)完成；完整兼容/视觉和默认 renderer 仍按后续阶段判断，未因测量完成调高预算。

## 早期检查点与清理

以下保留初轮和 2026-11-05 的状态及失败，不代表当前进度；后续待测量项已按[2026-11-11 综合验收](completion-2026-11-11/README.md)补齐。历史故障原因、预算差异及未覆盖环境仍保留，不将早期失败改标成功。

早期检查点中 RS-12 **尚未标为已验证**：三份候选已有暖启动字符记录，Wails 虽匹配物理热键但用户报告闪现隐藏/不能输入；当时冷样本、DPI/帧与计时未补齐，不能证明性能目标。2026-11-05 新增同 100 ms 的完整树启动/扫描观察，原始字节、扫描覆盖与采集开销见[续验](followup-2026-11-05/README.md)，仅为采样峰值下界；早期离散根进程[启动观察](startup-observation.json)保留原范围，不改标为完整树瞬时峰值。原 Computer Use 失败/实体 Esc 停止记录保留，合成输入不计物理按键。

已准备九份独立的未来冷样本副本并核对 exe/配置，启动器实测拒绝同一启动周期准备的 case，没有生成冷启动样本或自动重启。系统实际 Run 只读核对为没有 Velo 登录项，未改注册表，见[冷条件预检](cold-preflight.json)、[副本与保护检查](cold-prepared.json)及[桌面补验](../../rust-slint-rs12-testing.md)。

初轮测量进程正常退出。另一次未测普通 FemtoVG 窗口等待实际预热确认未完成，先按 PID/创建身份核对，再让所属托盘服务执行销毁清理，随后终止该测试进程；记录 `ForcedCleanup=true`、零资源样本，不当作正常退出或无探针性能证据，原[清理记录](cleanup.json)保留。用户之后回复未看到旧窗口，重新启动的两份普通实例经实际预热完成可见采样，均由用户正常退出、退出码 0；随后隐藏/实际输入轮次亦正常退出。最新状态、用户暂离时跳过项和清理见[剩余验证](remaining/README.md)，此前[可见续验清理](normal/cleanup.json)保留原时间。没有替换正式 exe、改登录项、提交或推送代码，默认 renderer 仍未决定。

PrivateBytes 是私有提交；WorkingSet 包含共享页，私有工作集和 GPU 内存单列，缺失字段保持 null。合成管道往返、组件模型更新、日志 ready 及工具调用耗时均不能证明搜索结果帧已呈现或搜索框实际接收字符。严格冷启动、实际按键、完整视觉/动效和渲染器选择仍按计划各自门槛验收。

2026-11-06 final verification: Go shared regression under Go 1.25.6 and full Rust verification with smoke tests for both locked renderers passed. Wails right-display native DPI round trip is observed with user-reported normal layout/input; physical hotkey capture remained incomplete at two valid bounds, two failed starts, and three unpaired character events, so reliability remains unpassed. Cold samples remain 0/9 by user decision; no Wails screenshot, frame timing, or physical IME evidence is claimed.

2026-11-08 automated continuation: both locked Rust renderers passed full verification and smoke, the Go 1.25.6 legacy entry and latencyprobe-tagged Go suite passed, and all RS-12 observer regressions passed. This run added no physical or performance samples. The 2026-11-07 physical IME pass remains scoped to this machine at 96 DPI; strict cold remains 0/9 pending an independent restart, Wails hotkey reliability remains below 20 valid samples, and calibrated input/frame timing remains open. See [the 2026-11-08 verification record](completion-2026-11-08/README.md).

2026-11-08 cold attempt: after a user-attested independent restart with no Velo auto-start, the frozen `software-1` binary exited normally with its prepared SHA-256, but its log had no accepted character and recorded `lifecycle_hidden reason=launch`. The collector preserved this as an invalid attempt; it does not count toward 0/9 valid samples. The boot claim is consumed, the next scheduled case is `femtovg-1`, and one additional independent Software case will be needed to reach three valid Software runs. The user did not notice whether an external application opened and reports it is now closed; the launch target is unknown. See [the attempt record](completion-2026-11-08/cold-software-1.json).
