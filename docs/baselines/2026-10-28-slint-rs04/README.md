# RS-04：采集适配、渲染器首轮 A/B 与 G1

执行日期：2026-10-28（北京时间）。对应 [Rust + Slint 重构方案](../../rust-slint-refactor-plan.md)的 RS-04。

**状态：已验证，G1 通过。** 两种渲染器完成构建、真实页面/输入观察及首轮 A/B；Software 达到固定原型的 30 MiB 私有提交预算，FemtoVG 未达到。可以进入 RS-05 的 Windows 生命周期验证；完整业务仍须先通过 G2。本目录中的截图不参与空闲资源测量。

## 构建身份

分支 `codex/rust-slint-refactor`，HEAD `ac2f51ad1317f4abffab2c548af251e48ce16d2b`，沿用已有 dirty 工作区，没有提交或替换 Go/Wails。Rust/Cargo 1.99.1、MSVC 14.50.35717、SDK 10.0.26100.0 和依赖/features 延续 RS-03，没有新增 Cargo 依赖。Rust/UI 编译输入和两条路径的共同设置一致，完整指纹保留在身份文件。

下表是原始 36 个资源样本、6 次实际输入和 12 张截图对应的构建，身份取自采样时保存的报告，不能用重建后的 manifest 覆盖：

| 渲染器 | 字节数 | SHA-256 | 身份 |
| --- | ---: | --- | --- |
| FemtoVG | 10,001,920 | `59867920F4EB17170AADA43AC29AEE387C5C209E9D19A2B751910EBEC21C4777` | [构建](femtovg-build.json) |
| Software | 10,503,168 | `214DC140513322B99301867F52B54F216BF9F7E8AD45CFCF8765237CD4AA6C74` | [构建](software-build.json) |

收尾刷新身份时触发了重新链接：Rust/UI/Cargo 编译输入未变，产物大小相同，但 SHA-256 改变。原测量 exe 未在重建前单独留存，原身份和原始样本仍完整保留；当前 `rust/target/artifacts/` 为重建程序，另冻结到忽略的 `.reasonix/rs04-frozen/`。重建 [FemtoVG 身份](femtovg-rebuild.json) / [Software 身份](software-rebuild.json)分别对应 `CCA988D943D18772464260352A60E7D92A8766425B6813C939D5ECD0AA34F654` / `195B5A7CC44B0C3C27243103760377742939E0C9C4B8A99CE366570458E58A03`，各补采 3 个可见样本，单独分组，不把旧样本移标到新 exe。字节复现原因仍需在 RS-14 拆查；构建成功不等于位级复现。

## 采集实现

- [measure-resources.ps1](../../../scripts/measure-resources.ps1)默认仍使用原来的 Wails 行为。`-ApplicationKind Slint` 要求 PID、实际观察到的十六进制 HWND 和匹配 exe 的 Rust 构建身份；同时核对 PID 启动时间、两层独占 renderer feature、文件 SHA-256 和固定 fixture 指纹。没有原生窗口时保持 Unknown，不能把句柄丢失当作隐藏。
- Wails 保持根进程及 WebView 后代的口径，排除已启动的外部应用；Slint 包含全部后代。保留原来的 CPU、可见性、状态连续性和数据稳定性判定，并增加私有工作集、线程、句柄、GPU Process Memory 和每次轮询的原始数值。缺失字段为 null，不能当作零。
- [measurement.rs](../../../rust/src/measurement.rs)只在显式 `--measurement-log=PATH` 时记录原生窗口、首次重绘请求、FemtoVG 首个 OpenGL 帧、焦点及真实 TextInput edited 回调。日志没有实际查询文本、用户路径或产品数据，写入通过容量 128 的工作队列。日志不得覆盖，丢失记录计数进入有效性检查。
- `--measurement-stdio` 另启用继承的匿名管道，阻塞等待采集器命令。没有轮询计时器或监听端口。hide/show 仅用于本项受控实验，复用同一个 winit HWND 与 Slint 组件，隐藏时关闭 presented 动画状态；不代表 RS-05 的正式热键/托盘/失焦生命周期已经实现。
- [collect-latency.ps1](../../../scripts/collect-latency.ps1)新增 `-ApplicationKind Slint` 分发到 [collect-slint-latency.ps1](../../../scripts/collect-slint-latency.ps1)，原 Wails 的 Prepare/Arm/Capture/Export 等路径保持兼容。Slint 使用 Arm → Capture → Computer Use 实际输入 → 正常关闭 → Snapshot/Export；要求原生和输入控件焦点、非合成字符、正确 PID 及无丢失事件。ready/窗口标题不算实际输入成功。
- [measure-slint-ab.ps1](../../../scripts/measure-slint-ab.ps1)统一驱动三种状态、3 × 15 秒样本和显隐循环；采集器在整个隐藏等待期每秒核实原生可见性。窗口不满足预期、进程变化或元数据变化时拒绝样本。

## 测量口径与范围

两份程序使用相同 Rust/UI/fixture 源码、Slint 1.18.1、winit、静态 CRT、Release 优化及嵌入资源，只改变必要 renderer feature。沿用既有未提交工作区，不提交或替换 Go/Wails 程序，不访问 `%LOCALAPPDATA%\Velo`。

固定 28 项，包含嵌入图标、中文、长名称、长路径和缺失图标回退。资源采样前让首页滚动到末项再返回首项，实际渲染各分组文字与图标；动画结束后开始计数。原生系统 125%，800 × 675 物理客户区对应 640 × 540 逻辑像素，不设置 SLINT_SCALE_FACTOR。不在资源采样期间截图，不运行 Cargo、Go 或前端构建。

每种渲染器在可见、至少隐藏 30 秒和至少隐藏 5 分钟时各采 3 个 15 秒样本；第二段隐藏等待独立连续观察满 300 秒，实际隐藏年龄更长。随后运行 100 次诊断显隐，再采 3 个可见样本。短期循环不能证明 30 分钟无增长，后者仍属于 RS-12。

资源顺序为完整 FemtoVG → 完整 Software → 新进程 Software 可见 → 新进程 FemtoVG 可见，形成正反顺序检查。再分别用无 measurement/diagnostics 参数的普通原型采 3 个可见样本；用真实 Up/Down 导航预热首页末项和首项。普通模式在报告中沿用 `Production` 枚举，但仍然是固定数据原型，不代表完整产品。

PrivateBytes 是私有提交，WorkingSet 包含共享页，私有工作集另列。GPU 的 Dedicated/Shared/TotalCommitted 来自 Windows GPU Process Memory 提供程序，单列报告，不能简单与进程 PrivateBytes 相加。应用尚无正式解码 LRU；Slint 内部字形/纹理缓存字节无法直接观察，不能宣称已验证 8 MiB 图标缓存预算。

本轮是固定原型可行性测量，没有完整业务扫描或正式应用图标缓存，不与历史 Wails 数据计算百分比收益。严格冷启动、热键呼出 P95、完整搜索性能、30 分钟长期增长及全功能同轮 Wails 对照仍属后续阶段。

## 资源结果

原始构建共 36 个有效 15 秒样本，重建补验另有 6 个；均为单个 Rust 进程，没有辅助子进程。数值为各组 3 个样本端点的中位数，单位 MiB；每秒原始字节、全轮询范围、CPU 有效性、DPI、线程/句柄和进程身份见 [资源汇总](resources-summary.json)及各组 JSON。可见性不明或进程变化的样本不会进入汇总。

| 状态 | FemtoVG 私有提交 | FemtoVG 私有工作集 | FemtoVG CPU % | Software 私有提交 | Software 私有工作集 | Software CPU % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 可见，首轮 | 137.21 | 60.41 | 0.037992 | 6.82 | 5.86 | 0.004227 |
| 隐藏至少 30 秒 | 137.08 | 59.29 | 0.042723 | 6.70 | 5.79 | 0.042744 |
| 隐藏至少 5 分钟 | 136.89 | 59.38 | 0.042592 | 6.70 | 5.83 | 0.038051 |
| 100 次显隐后可见 | 143.91 | 64.99 | 0.033834 | 6.96 | 6.06 | 0.000000 |
| 反向顺序，新进程可见 | 136.79 | 59.56 | 0.025052 | 6.82 | 5.86 | 0.004257 |
| 普通原型，无探针 | 132.67 | 54.89 | 0.021132 | 6.62 | 5.75 | 0.000000 |

CPU 按 24 个逻辑处理器归一化；0.04% 全机约等于 0.96% 单逻辑核心，不能写成完全无后台活动。首轮可见 WorkingSet 中位数为 FemtoVG 110.57 MiB / Software 28.41 MiB；与上表两种私有指标含义不同。采集宿主为 32 位 PowerShell 7，程序和锁定目标均为 Windows x64；私有工作集来自 PerfProc 的 WorkingSetPrivate，GPU 来自 Windows GPU Process Memory，模块枚举使用系统 tasklist，未使用跨位数的 Process.Modules 结果。

| 首轮可见 GPU 计数 | FemtoVG | Software |
| --- | ---: | --- |
| Dedicated Usage | 45.91 MiB | null，无该 PID 实例 |
| Shared Usage | 19.95 MiB | null，无该 PID 实例 |
| Total Committed | 61.69 MiB | null，无该 PID 实例 |

这几种 GPU 口径不保证互不重叠，不相加推算总内存。FemtoVG 首轮可见 45–46 线程 / 601–602 句柄，Software 为 13 线程 / 285–286 句柄；完整范围均保留在原始资源文件。原测量的两种模式各完成 100 次循环，原生窗口记录各只有一条，队列丢失计数均为 0。

重建可见补验的 PrivateBytes 中位数分别为 FemtoVG **137.37 MiB** / Software **6.82 MiB**，分别见 [FemtoVG](femtovg-rebuild-visible.json)与 [Software](software-rebuild-visible.json)，并重新通过两条路径的窗口/组件输入冒烟。每组 JSON 自带实际 exe SHA 和构建身份；补验未重采隐藏五分钟或重复 100 次循环，不能挪用原构建的隐藏态样本。

## 输入就绪与截图

顺序采用 FemtoVG → Software → Software → FemtoVG → FemtoVG → Software，分别输入固定的英文匹配、中文匹配和空结果文本。每次只观察初始控件焦点，不点击搜索框；Computer Use 会激活目标窗口，因此这些记录不能证明后台启动或正式呼出的前台激活策略。真实微软拼音和原生 100%/125%/150%/200% DPI 证据沿用 [RS-03](../2026-10-28-slint-rs03/README.md)，本轮没有重复改变系统 DPI。

实际接受输入是结束边界，时间由进程创建 UTC 与应用接受事件 UTC 计算，应用内部阶段另保留单调时钟。Computer Use 操作/等待包含在时长内，方法名为 `ComputerUseAssistedUpperBound`，不把它当作校准过的暖启动性能，也不据此判定 500 ms 预算。汇总复用 [summarize-latency.ps1](../../../scripts/summarize-latency.ps1)的中位数、最近秩 P95 和缺项规则，ColdStartup/HotkeyActivation 保持缺失。

Software 的 `first_redraw_requested` 仅表示重绘请求，不能冒充已呈现首帧；实际绘制由独占 feature、窗口冒烟的非空白 snapshot 及独立实机截图共同证明。FemtoVG 另有 NativeOpenGL 帧观察。截图由 Computer Use 直接保存 JPEG，没有裁剪或重采样；本机归一化为 640 × 540，不能用于 RS-13 的物理像素误差验收。

| 场景 | FemtoVG | Software |
| --- | --- | --- |
| 浅色首页 | [截图](femtovg-light-home.jpg) | [截图](software-light-home.jpg) |
| 深色首页 | [截图](femtovg-dark-home.jpg) | [截图](software-dark-home.jpg) |
| 英文列表与长路径 | [截图](femtovg-light-list.jpg) | [截图](software-light-list.jpg) |
| 中文列表 | [截图](femtovg-chinese-list.jpg) | [截图](software-chinese-list.jpg) |
| 空结果 | [截图](femtovg-empty.jpg) | [截图](software-empty.jpg) |
| 首页末行、系统分组与缺失图标 | [截图](femtovg-home-bottom.jpg) | [截图](software-home-bottom.jpg) |

两种渲染器核心页面均可读，六列、焦点边框、长文本截断和空结果一致可用；字体栅格化和过渡中间帧不作逐像素等价承诺。截图等待有限动画结束后保存，不用截图证明动画性能。

## 检查与证据

已执行两种渲染器的 fmt、clippy `--all-targets -- -D warnings`、各 9 项行为测试、Release、窗口冒烟及各 15 项组件输入断言。新增采集检查覆盖构建身份/feature 隔离、实际输入边界、焦点/合成、日志丢失/PID 与缺失输入拒绝。共享脚本变更另执行 `./scripts/verify.ps1 -SkipBuild`：采集回归、前端 lint、26 项前端测试、前端构建、Go vet 和 Go tests 均通过；没有替换安装包。

原始本地日志保存在忽略的 `.reasonix/rs04-*`；公开证据保留每秒资源样本、构建身份及输入阶段，移除本地 exe 路径。输入证据为 [FemtoVG](femtovg-input-observations.json) / [Software](software-input-observations.json)，CSV 和汇总分别见 [FemtoVG CSV](femtovg-input.csv) / [汇总](femtovg-input-summary.json)及 [Software CSV](software-input.csv) / [汇总](software-input-summary.json)。每种 3 次接受输入，0 次失败；两个 ColdStartup/HotkeyActivation 缺项保留，汇总的 AllSampleCountsSatisfied 为 false。

自动检查、源码/采集脚本指纹与公开文件校验见 [validation.json](validation.json)、[provenance.json](provenance.json)和 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)。

## 问题、预算与 G1 判断

| 项目 | 结论与下一步 |
| --- | --- |
| 固定原型 ≤30 MiB 私有提交 | Software 通过；FemtoVG 失败。无探针 FemtoVG 仍为 132.67 MiB，超标不能只归于探针。Software 的结果只覆盖固定原型，不证明完整应用 ≤50 MiB 或较 Wails 降低 50% |
| FemtoVG 开销归因 | [模块证据](femtovg-modules.json)包含 NVIDIA OpenGL ICD、GPU 编译器和覆盖层模块，并有上述 GPU 分配及更多线程；[Software](software-modules.json)没有这些模块。证据指向渲染器/驱动上下文开销，但未把每一 MiB 分配归到驱动、Slint 或字形/纹理缓存。RS-12 增加受控上下文/资源预热分段与驱动环境对照；空白窗口仅可用于归因，不能代替预算样本 |
| 显隐后增长 | FemtoVG 相对首轮可见中位数增加 6.70 MiB，Software 增加 0.13 MiB。两段稳定可见时均恢复焦点，循环中发生失焦/重新激活；目前无法区分延迟缓存预热、驱动资源池与持续泄漏。RS-12 按循环分段并完成 30 分钟记录，不将这次端点差异标为无泄漏 |
| 隐藏态 CPU | 两种路径仍约 0.04% 全机，未宣称空闲完全为零。当前使用原生 set_visible(false) 的诊断隐藏，Slint 对象保留；RS-05 须验证正式隐藏状态、计时器/重绘暂停和恢复输入，进一步定位活动来源 |
| 图标/字体缓存 | 8 MiB 应用解码 LRU 尚未实现，内部缓存不可读，保持未验收。正式图标缓存属于 RS-10，含引用释放与 DPI 变化的资源验收在 RS-12 |
| 输入与延迟 | 实际中文/英文输入、焦点及核心页面可用；IME/DPI 关键实机证据延续 RS-03。当前暖启动只有工具辅助的输入上界，500 ms、热呼出 P95 50 ms、严格冷启动均未判定通过；RS-05/RS-12 继续补齐 |
| 位级构建复现 | 相同 Rust/UI 编译输入重建产生不同 SHA，已保留两套身份并分组补测。RS-14 检查生成代码/链接元数据等来源；后续采样应先冻结 exe 与身份，再测量，避免覆盖原程序 |
| 兼容与完整视觉 | 本机 Windows 11、微软拼音；Windows 10、第三方 IME、运行中/混合 DPI、多屏及完整业务页面仍按后续阶段实测。未以核心页面截图替代 RS-13 的完整视觉与动画验收 |

G1 的四项条件均满足：两条路径有可复现构建入口与首轮测量；中文、焦点及核心页面可用；原始资源样本和截图齐备；超预算与未完成的指标都有证据和下一步。没有发现阻断第一阶段输入的回归。**下一项为 RS-05**，继续保留两种渲染路径；默认渲染器仍留到 RS-14 决定。
