> 历史复现说明：本页命令和 `rust/` 路径对应整理前提交 `cc65619`。当前根目录工程的构建与验证见 [开发说明](development.md)。

# RS-12 桌面补验与复测

本页配合[重构计划](rust-slint-refactor-plan.md#6-第四阶段性能与视觉验收)与[本轮记录](baselines/2026-11-04-slint-rs12/README.md)。2026-11-11 [RS-12 本机测量矩阵已验证](baselines/2026-11-04-slint-rs12/completion-2026-11-11/README.md)，九份机器冷计时已补齐，不再需要为本矩阵重启。以下入口用于追溯和新一轮复测；只对原身份追加证据，不将历史原型或另一份 exe 的结果改标为当前程序。

## 已准备的入口

当时的构建与数据固化步骤见[历史 Rust 工程说明](https://github.com/HBLADEH/velo-launcher/blob/cc656198b53ed049ba6abb13ff6f76738e247004/rust/README.md#rs-12-完整业务采集)。迁移期 `prepare-rs12.ps1` 从官方发布取回并核验 Wails v0.9.11，冻结同源码的两个 Slint Release、原始数据副本、配置预设、磁盘图标和源码身份；这些准备工具已从当前主线移除。原始资料留在 `.reasonix/`；正式目录不作为测试写入目标。

每份程序单独启动，退出后再换下一份；Wails 除独立数据外仍共用原来的全局互斥锁/窗口类/热键，因此不能与正式版并行。Slint 同时指定独立实例 ID 和目录。启动脚本默认保持记录进程的创建身份及正常退出码，不修改登录项、系统显示、电源或更新安装设置。

```powershell
# 每轮 RunName 都使用新名称。此命令持续等待托盘/应用内正常退出。
./scripts/start-rs12-session.ps1 -PreparedDirectory .reasonix/rs12-new -Edition wails -RunName wails-manual-1
```

`Edition` 另可选 `software` / `femtovg`。普通桌面采集使用实际输入；Slint 的手动会话开启只观察的日志，不开启 stdio 合成负载。窗口由鼠标或快捷键激活后，用可见状态和实际字符确认焦点，不凭进程存在判断输入已就绪。

需要无探针对照时，启动脚本加 `-Uninstrumented`。Slint 使用相同 exe，既不启动日志线程，也不启动控制线程；采集器通过 PID/原生类名只读定位实际 HWND。因为没有内部事件，只有在操作者确认扫描结束并完成同场景预热后，才使用 `OperatorBusinessConfirmed` 保存人工证据；无法读取的目录状态和应用缓存数值保留 null，和自动矩阵分组。

## 资源与交互

先确认扫描结束并记录警告，保持同一屏幕、DPI、输入法、主题和列表布局。滚动预热首页全部行，再搜索一次英文与中文，清空回首页，等动画停止。采样期间不构建、运行基准、截图或操作待测程序。

在另一个采集终端中运行，窗口由操作者先设为目标状态：

```powershell
./scripts/measure-rs12-session.ps1 -SessionDirectory .reasonix/rs12-new/runs/wails-manual-1 -Phase visible -IdleEvidence '填写本次实际扫描完成、首页图标与中英文查询预热的观察依据'
# 实际隐藏后，再顺序执行；第二次隐藏从新的显隐动作开始。
./scripts/measure-rs12-session.ps1 -SessionDirectory .reasonix/rs12-new/runs/wails-manual-1 -Phase hidden-30s -IdleEvidence '填写同一会话的实际预热依据和隐藏操作'
./scripts/measure-rs12-session.ps1 -SessionDirectory .reasonix/rs12-new/runs/wails-manual-1 -Phase hidden-5m -IdleEvidence '填写同一会话的实际预热依据和隐藏操作'
```

每项包含连续状态观察和 3 × 15 秒样本。`AllSamplesEligible=false` 的原文件保留，用新会话重试。检查完整进程树、PrivateBytes、WorkingSet、私有工作集、CPU、线程/句柄和 GPU；采不到的字段保留 null。比较扫描后实际有序应用集合/数量，不能只因最初 seed 一样就忽略后台发现产生的差异。

另保存首次图标加载/缓存命中、中文预热、快速输入、设置往返、真实 DPI 转换及至少 100 次显隐/30 分钟操作空闲记录。Slint 自动入口产生的是组件/生命周期合成操作，不能写成真实键盘、IME 或 DPI 变化。原生窗口出现、组件模型更新和像素帧呈现分别记边界。

人工会话可使用 `observe-rs12-session.ps1 -SessionDirectory <会话目录>` 保存 20 + 10 个一分钟资源样本，观察期间由操作者进行实际查询、显隐和页面操作。此入口只读取进程/窗口，不发送按键，不传 `IdleConfirmed`；包含显隐变化的样本按操作观察保存，不能改标为空闲基线。一秒窗口轮询会遗漏快速变化，真实操作次数仍需操作者记录；它也不提供校准后的按键到输入延迟。无效的三态空闲组可用 `measure-rs12-session.ps1 -Attempt 2` 或 `-Attempt 3` 在同一进程重采，旧文件保留。

`measure-rs12-session.ps1` 要求显式 `-IdleEvidence`，不能由脚本自动生成“已预热”声明；未完成实际预热时只做不带空闲确认的观察。Wails `latencyprobe` 通过环境或 sidecar 启用，不能仅凭命令行没有诊断参数就称为普通模式；资源入口根据 Go 构建标签记为 `ProbeBuild`，可选观察器是否实际启用另核对会话。2026-11-05 曾因此错误标记的三份新样本已整体排除，原记录保留，不混入原 54 份有效空闲基线。

## 启动和热键

帧观察的新增入口与失败边界见 [2026-11-10 续验](baselines/2026-11-04-slint-rs12/completion-2026-11-10/frames/README.md)。先用 `build-rs12-observer.ps1 -OutputDirectory <全新构建目录>` 构建外部工具，并运行 `test-rs12-frame-native.ps1 -ObserverExecutable <工具exe> -OutputDirectory <全新自校验目录>`。该工具使用锁定 MSVC/SDK，产品不增加依赖。`observe-rs12-frames.ps1 -SessionDirectory <实际输入会话> -ObserverExecutable <工具exe> -OutputDirectory <全新目录> -Seconds 180 -SaveFrames 24` 只观察指定 HWND 内缩裁剪和被动按键；位图留本地，默认不保存图像。隐藏/遮挡或几何尚未稳定的帧排除，旧几何捕获只能显式作为开发诊断，不能改标为产品证据。采集前先确认操作者就绪，在有界捕获内正常退出；未退出、时钟变化或图像缺项均保持待验。

正常退出后使用 `summarize-rs12-frames.ps1 -CaptureDirectory <捕获目录> -SessionDirectory <同一会话> -OutputPath <新JSON>`。桌面 `LastPresentTime`、GPU 读回完成与物理显示器呈现是不同边界，像素 hash 变化也不证明结果文本正确；仍需核对预期结果与实际帧。整段 UTC/QPC 跳变默认拒绝；显式 `-ClockSegmentStartQpc` 必须对应原时钟括号，区间外按键保留为无效，不得无声裁掉失败。受控规模由 `prepare-rs12-frame-fixtures.ps1 -PreparedDirectory <冻结构建> -BaselineIndexes <本地索引副本...> -OutputDirectory <新目录>` 准备，正常发现保留并核对实际发布数量；第三版合法图标夹具与首轮回退图标条件分别记录。

进一步实测发现 Rust 发布目录时会重新计算合法 URL 的图标签名，第三版仍可能显示回退字母。当前第四版通过 `cmd/rs12-icons prepare-frame-icons` 复用真实 `icon.Populate`，为前 16 个首页/搜索可见项生成路径/源文件时间对应的 cache PNG；测试必须确认实际图标加载，不能只检查 URL 语法或源 PNG 可解码。[六条件补验](baselines/2026-11-04-slint-rs12/completion-2026-11-10/frames/corrected/matrix-final/README.md)已完成 33 次固定 A 查询、结果参考/桌面动效和正常清理。额外两条编辑和之前空采集都保留；这些结果不验收精确首次可输入或每帧目标 CPU/GPU。

每份程序至少 3 次暖启动和 20 次真实热键。输入就绪的结束边界为搜索框实际接收字符；只看 ready 日志、DOM 可写、窗口可见或组件焦点都不足以验收。关闭旧实例后从相同 seed 新建会话，保留系统缓存，暖启动不能称为冷启动。

采用覆盖“按键/进程创建 → 实际输入”的校准桌面观察或高帧率录像，保存原始片段、时钟分辨率及误差。人工等待、工具激活和输入调度时间单列；无法扣除的等待只能报告观察上界，不能据此认定 <500 ms/<50 ms 达标。失败/超时记录保留，分组计算中位数、最近秩 P95、范围和失败数量；不同程序 SHA、方法和边界不能混合。

旧 `collect-slint-latency.ps1` 继续服务 RS-04 固定样例，不作为本轮完整业务的一键计时入口。旧 Wails 探针只确认可写焦点，不能代替本轮真实字符及搜索呈现边界。后续原生机器校准入口已完成暖启动/机器热键和查询动画 CPU/桌面帧观察；方法与限制见下文，不改变旧观察的范围。

RS-12 已新增独立的实际输入入口：`prepare-rs12-input.ps1` 固化原 Slint exe 和另一个 `latencyprobe` Wails 构建；Wails 的新输入桥只在 `VELO_LATENCY_ACTUAL_INPUT=1` 时记录 trusted/composing/字节数，旧的 ready 记录保持原语义。`start-rs12-input-session.ps1` 通过独立 ID/数据启动实际业务实例；`collect-rs12-input.ps1 -Kind HotkeyActivation` 用被动 hook 观察隐藏状态的物理 Alt+Space，拒绝注入起点，并匹配实际非合成字符。`export-rs12-input.ps1` 正常退出后重新检查完整日志的丢失状态、保留所有失败，再调用既有汇总器。该入口不读取查询文字或其他按键，观察上界仍含人工打字等待，不能单凭它验收首次可输入的 <50 ms 目标。

`prepare-rs12-warm.ps1` 创建实际快捷方式与三份输入副本，`collect-rs12-warm.ps1` 在用户从资源管理器启动后读取 OS 创建时间和实际接收回调。Wails 必须用正常退出的独立测试 `webview` 缓存副本，通过 `-WebViewCacheSource` 指定，避免把新建 WebView profile 算作同一种暖启动。采集器只保存 1 ms 的额外 guard，并未独立校准 FILETIME/UTC 步进误差，不应写成 1 ms 精度。当前两种 Slint 的实际输入范围、失败与限制见[续验](baselines/2026-11-04-slint-rs12/remaining/README.md)；Wails 实际轮次在用户暂离期间跳过。

2026-11-05 Wails 人工补验发现闪现隐藏/不能输入，原成功匹配数没有覆盖全部人工尝试，见[续验问题记录](baselines/2026-11-04-slint-rs12/followup-2026-11-05/README.md)。不要让五次起点的开发诊断代替完整人工操作窗。需要核对失败时使用 `collect-rs12-input.ps1 -Kind HotkeyActivation -CountMode UntilExit -TimeoutSeconds 900 ...`，在就绪后开始操作，失败时保留十秒超时，再从托盘正常退出。此模式持续至目标窗口结束或超时，不以成功数提前结束；`Completed` 与有效次数、人工反馈、最终正常退出分别评审。重试和注入按键分别记录，不采查询文本；显隐/前台回调时间不是帧呈现或组件输入焦点。

2026-11-09 [二十轮 Wails 续验](baselines/2026-11-04-slint-rs12/completion-2026-11-09/wails-hotkey/README.md)取得 20/20 物理起点与可信、聚焦、非合成字符配对，用户报告全部正常；此前间歇性失败仍未归因。该次旧采集器未识别窗口销毁而超时，辅助窗口报告丢失；有效字符数、原始失败和正常退出分别保留。当前观察器检查 HWND 查询返回值和进程退出，保留销毁前的完整观察；仍有效窗口的几何读取失败继续报错。包装脚本在加载观察器前保存源码哈希，避免采集期间编辑文件使最终报告误标身份。采集失败提示同时报告结束原因和有效样本数，不用退出码代替样本/稳定性评审。

启动扫描资源可通过 `start-rs12-session.ps1 ... -StartupResourceSeconds 30 -StartupResourceIntervalMilliseconds 100` 采集；采集器编译在应用启动前完成，任务紧随进程创建启动，报告第一快照间隙。`measure-rs12-peak.ps1` 也支持现有会话的有界捕获。逐次完整树的最大值只是采样峰值下界，应检查扫描起止、实际轮询间隔、不完整快照与采集器 CPU；不与稳定空闲或严格冷启动混用。20 ms 开发轮开销较高，保留原样本并以 100 ms 作为后续对照入口。

DPI 场景必须核对目标 HWND 的原生 DPI，不能仅凭系统设置选中 150% 或脚本阶段名判定。2026-11-05 的 FemtoVG 首轮在另一屏缩放后仍留在主屏 96 DPI，原记录排除并单独复验，见[桌面记录](baselines/2026-11-04-slint-rs12/followup-2026-11-05/desktop/README.md)。工具截图可被归一化为逻辑窗口尺寸，物理尺寸以原生记录为准；截图采集时间不提供首帧或输入到呈现耗时。工具送入字符即使被进程记为 `synthetic=false`，也应按自动化输入保留来源，不计物理热键/IME。

## 严格冷条件

暖启动/呼出的新增机器校准、动画CPU和最终待验边界见[暂离自动补验](baselines/2026-11-04-slint-rs12/completion-2026-11-10/remaining-auto/README.md)。`build-rs12-calibrator.ps1` 构建固定Unicode输入的原生集成探针；`measure-rs12-readiness.ps1` 使用原冻结exe/新隔离seed，启动前预先就绪，带PID/创建时间/exe/HWND/实际前台焦点保护，缺失字符、焦点拒绝和时钟跳变均保留。最终采用私有高精度timer，不改变全局timer period。所有记录标注机器来源，不能仅凭产品 `synthetic=false` 写成物理键盘，也不能替代已有真实IME/热键验收。`measure-rs12-animation.ps1` 另采500ms查询动画窗口的完整树CPU原始计数和桌面帧，原始量子/采集开销与逐帧GPU执行分开。

机器冷计时通过 `prepare-rs12-calibrated-cold.ps1` 预冻结原exe/数据、工具和来源。最终集合已完成 [9/9](baselines/2026-11-04-slint-rs12/completion-2026-11-10/calibrated-cold/README.md)，三个候选各 3/3；Software/FemtoVG 六份接收上界均在 500 ms 内，最近秩 P95 为 287.492/473.743 ms，Wails 三份均未验证该预算、较慢样本保留。这九份 case 均已使用，不再运行或覆盖；未来复测须提前准备新集合，用户另一次独立重启并确认本周期未运行 Velo 后，才对新 case 调用 `start-rs12-calibrated-cold.ps1 -CaseDirectory <新case目录> -FirstVeloLaunchThisBootConfirmed`。退出可由操作者或已隔离实例的原正常退出入口发起，记录来源，不把接口退出写成物理托盘操作。沿用全工作区 boot ledger，不可换集合重复计同一周期；测量前只读元数据和外部探针，不哈希/映射候选 exe 或读取应用输入。`export-rs12-readiness.ps1 -ColdCases` 保留独立 boot ID/机器来源，拒绝有效冷样本共用 boot，输出始终使用新文件名。原人工字符 9/9 与机器冷计时分别统计，不能扣除旧人工等待后重新命名为精确冷耗时。

三份程序各至少 3 个独立冷条件，共至少 9 个系统启动周期；每周期最多一份程序的第一次启动计冷启动。后启动的其他程序属于暖启动。每次记录系统启动 ID、实际登录启动入口及启用状态、是否已有 Velo/测试程序运行，保留原始证据。

不要为测量自动重启电脑或改登录项。由操作者安排重启与会话，必要的临时登录项变更另行明确并保留可恢复原值。某一周期若已运行候选或预读其程序/输入文件，就不能再补写成该周期的严格冷样本；缓存文件冷/热也不是系统冷/暖。

可提前运行 `prepare-rs12-cold.ps1 -PreparedDirectory <固化目录> -OutputDirectory <新冷样本目录>` 为三份程序各建立三份隔离副本。实际输入验收使用 `prepare-rs12-input.ps1` 产生的测量构建目录；原官方 Wails Release 不记录实际接收回调，旧 case 只能提供进程存在观察。后续独立重启后，人工核对登录入口和本次首次运行条件，再用 `start-rs12-cold.ps1 -CaseDirectory <某一份副本> -FirstVeloLaunchThisBootConfirmed` 启动。启动器拒绝同一系统周期准备的 case、正在运行的 Velo，并在工作区 `.reasonix/rs12-cold-boot-ledger` 共用 create-new boot claim，另建 case 集合也不能重复使用同一周期；测量前不哈希/映射 exe，也不读应用输入文件。

正常退出后使用 `collect-rs12-cold.ps1 -CaseDirectory <同一副本>` 检查独立 boot claim、原 exe/退出身份、无丢失且实际聚焦的非合成字符。没有实际输入或异常退出仍保存无效尝试。首次 Velo 运行条件依赖人工核对，启动后的实际接收、录像/独立时间校准和冷条件证据需要分别评审；FILETIME/接收 UTC 加 guard 的观测上界不能当作首次可输入的精确冷启动耗时。不能把启动脚本退出 0 当作冷延迟达标。

2026-11-09 起的新冷报告使用 `ColdWindowsBuildCpuOrdinalV1`：Windows 版本、构建号和处理器名称按固定顺序序列化，处理器名称按 ordinal 顺序排列，再计算环境 ID。此前无序字段可能使相同信息产生不同 ID；历史原件保持原样，比较时须单独核对，不能只凭旧 ID 是否相等判断环境是否一致。环境 ID 仅覆盖上述字段，不能证明其他测试条件相同。

## 结束与证据

从应用内、托盘或隔离测试实例的正常生命周期入口退出并记录来源；核对测试 PID/创建时间、实例端点、托盘和快捷键清理。具备启动前正式文件哈希时核对前后原文字节；冷条件未做该预读时只保留参考比较、退出后哈希及最后写入时间，不能声称本轮连续未变或归因变化。公开 JSON/CSV 脱敏路径/身份，保留 exe、源码、输入、采集器和本地原件哈希。填写结果与未覆盖边界后更新阶段状态；RS-13 视觉与 RS-14 渲染器/发行验收各自进行。

2026-11-06 的 Wails 历史热键只有四个物理起点，不能改标成二十轮通过；原生 96/144/96 DPI 和用户布局/输入反馈保持原范围。后续三候选物理 IME、各至少二十次物理热键、九份人工冷字符观察，以及机器暖/热/冷校准、六条件查询/动画观察已完成，综合结论见[最终验收](baselines/2026-11-04-slint-rs12/completion-2026-11-11/README.md)。原失败、人工等待、合成状态与再次激活记录不被覆盖；历史失焦原因仍未知，阶段测量完成不表示总体发行稳定性通过。
