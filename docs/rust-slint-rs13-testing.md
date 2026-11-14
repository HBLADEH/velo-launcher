# RS-13 视觉续验

2026-11-13 [本机验收已完成](baselines/2026-11-07-slint-rs13/completion-2026-11-13/README.md)：当前两份exe的100/125/150/175%微软拼音、布局及跨屏复核已通过，用户接受列明的外观差异，显示恢复100%。当前原生设置进出有7段满足原门槛，FemtoVG录制脉冲补齐；历史误差超限/缺帧尝试保留，200%仍为不可选未测，范围不扩大到其他系统/驱动/IME，默认renderer/G4仍属RS-14。下文保留11月12日的历史检查点；本机无需再重做这批人工步骤。

2026-11-12 [原生补验](baselines/2026-11-07-slint-rs13/completion-2026-11-12/autonomous-followup/README.md)覆盖三候选各14类/48段原生录像、48项局部动态和43张原生页面。之后的[当前构建续验](baselines/2026-11-07-slint-rs13/completion-2026-11-12/current-build-followup/README.md)先确认这批录像属于157输入旧构建，再按当前两个206输入exe身份录制设置进出和快捷键录制状态。四段设置过渡的活动帧较密，但首个中间帧前的间隔为350ms以上，故触发时刻/曲线仍为样本不足。Software录制ROI两个高位区间相隔1184.509ms，接近1.2秒声明；父追踪后来达到128 MiB上限，已保留原限制标记。自动化窗口操作不替代新exe的物理IME/DPI证据，之前矩阵继续保持原构建身份。

实施门槛以[重构计划 §6.2](rust-slint-refactor-plan.md#62-视觉与动画基线)为准。组件快照、原生桌面、真实输入法分别留证；更换 UI 后不能把 RS-12 冻结程序的测量身份改成当前程序。

## 可重复的自动采集

在仓库根目录使用锁定工具链，保持其他 Velo 和采集窗口退出，输出目录均须全新：

```powershell
$env:GOTOOLCHAIN = 'go1.25.6'
./scripts/verify-rust.ps1 -SmokeTest
python scripts/generate-rust-rotating-icons.py --check

./scripts/capture-rust-visual.ps1 -OutputDirectory .reasonix/rs13-static-my-run
python scripts/test-rs13-visual.py .reasonix/rs13-static-my-run .reasonix/rs13-heading-my-run.json
python scripts/summarize-rust-visual.py .reasonix/rs13-static-my-run .reasonix/rs13-static-gallery-my-run

./scripts/capture-rust-animation.ps1 -OutputDirectory .reasonix/rs13-animation-my-run
python scripts/summarize-rust-animation.py .reasonix/rs13-animation-my-run .reasonix/rs13-animation-gallery-my-run

# 校准原生曲线分析器的源样本边界；输出文件同样必须全新。
python scripts/test-rs13-transition-contract.py .reasonix/rs13-transition-calibration-my-run.json
```

静态程序覆盖深浅主题各 26 个场景，包含多行与滚动底部。动画程序使用合成组件事件，录制入场、业务设置/导入开关、hover/按压、齿轮、扫描、录制以及减少动画/隐藏场景。时间戳来自实际快照时刻；无损 WebP 按实际间隔播放。快照和编码会增加开销，不能用其间隔认定显示器帧耗时或产品 CPU/延迟。

原始 PNG、失败尝试和 manifest 留在本地；公开材料保留 exe、UI、夹具与工具 SHA。不同渲染器分别构建，禁止沿用请求的 renderer 名称去标记静默回退结果。

Software 1.18.1 不支持图像变换，旋转中的刷新与设置图标由 `generate-rust-rotating-icons.py` 将原 Fluent SVG 的 Bézier 路径转换为 `rust/ui/rotating-icons.slint`，在原控制点上计算旋转；静止时仍使用原 SVG。`--check` 核对轮廓来源，没有新增位图序列、缓存或依赖，仍遵循 `rust/ui/assets/fluent/LICENSE.txt` 的 MIT 许可。

## 原生同轮对照

使用包含 `Builds`、`SeedDirectory`、`Inputs` 的合成参考 manifest。每个候选均固定 exe SHA，Slint 两份须通过完整验证，Wails 保留 v0.9.11 的实际构建身份。准备入口会逐项核实 seed 与当前 Rust 构建的源码/验证回执，再创建深浅两份独立 seed。先退出所有候选和组件采集窗口，再逐一运行：

```powershell
./scripts/prepare-rs13-visual.ps1 -ReferenceManifest <合成参考manifest> -OutputDirectory <全新准备目录>
./scripts/start-rs13-visual-session.ps1 -PreparedDirectory <全新准备目录>/light -Edition wails -RunName wails-light
# 正常退出上一份后，用 software / femtovg 分别启动。
```

选定窗口的原生录像可附加外部采集器：

```powershell
./scripts/build-rs13-window-recorder.ps1 -OutputDirectory <全新采集器目录>
./scripts/start-rs13-visual-session.ps1 -PreparedDirectory <准备目录>/light -Edition software -RunName software-motion -FrameRecorderExecutable <采集器目录>/rs13-window-recorder.exe
# 在该 run 的 native-frames/clip.txt 写入新的短名称，再操作窗口；每段最多 128 帧/128MiB。
python scripts/summarize-rs13-native.py <run>/native-frames <run>/session.json <全新公开目录>
```

WGC 保存选定 HWND 的客户区内侧、源帧时间及观察 QPC，不捕获整个桌面或鼠标。`Published=true` 只表示生成预览，不能作为验收；存储受限、前台中断及过大帧间隔须另行排除。保留原始时钟/帧 SHA；原生精确时长、轨迹需要足够密集的采样，不能用片段总长、组件快照或 CPU 括号代替。退出后核对产品与观察器各自的完成文件。

此入口复制 seed 到该轮独立 `local/Velo`，Wails 重定向 `LOCALAPPDATA`，Slint 同时隔离实例与目录。Wails 的 `--diagnostics` 防止截图时失焦隐藏；Slint 的同名参数只增加日志，仍保留正常失焦隐藏。不得据诊断场景重新验收生产失焦/热键可靠性。测试不得指向正式 `%LOCALAPPDATA%\Velo`。不启动候选应用，不使用关机、锁屏、清空回收站等系统动作来制作视觉状态。

按相同数据、主题、查询、选中项、滚动位置和逻辑尺寸核对首页、空固定组、列表、六列网格、空结果、长名称/路径、自定义应用及七个设置页。设置的索引/备份/关于滚动到底，检查许可仍可见；展开主题/排列下拉；打开候选项右键菜单。原生选择器只检查外观与取消；不选中正式文件。更新与错误使用合成状态，不为截图下载或安装。

保存 PID/创建时间、exe SHA、输入 SHA、原生 HWND/DPI/客户区几何、截图来源和操作类型。采集窗口不得相互遮挡。Computer Use 返回的 JPEG 是原始有损窗口截图，可用于页面检查；严格边界签收需要另存无损客户区截图并校验裁剪原点，不能把 JPEG 差异直接当作像素容差。

在 100% 原生 DPI 上量取搜索框、分组/首行、列表、页签、表单和底部按钮边界，原则上误差 ≤2 个逻辑像素；不通过缩放对齐掩盖几何差异。文字抗锯齿单独说明。阴影/渐变、组件样式、文案/布局或原生菜单的不同均须逐项记录，未获接受的差异保持待验。

## 用户在场时补验

- 用当前两种 Slint 构建及同轮 Wails 对照，复核真实 100/125/150/200% DPI；有混合 DPI 双屏时拖动跨屏并检查窗口/控件位置，结束后恢复原显示配置。`SLINT_SCALE_FACTOR` 只能作为开发覆盖，不能代替原生 DPI。
- 使用微软拼音物理键盘输入，观察候选框定位、翻页、选词、取消和中英切换；合成期间 Enter/Space/方向键/Esc 不启动、不导航、不隐藏。普通的自动化 Unicode 文本不计入此项。
- 保留同场景原生动画录像，逐帧核对两行 tile 的每列 24 ms 节奏、内容入场、hover/按压、220/280/100 ms 面板过渡，以及扫描 1 s/录制 1.2 s。动画 CPU/帧观察与稳定空闲分开。
- 查看选择器及适用的原生确认提示。涉及 UAC、安全桌面或系统权限时由用户操作；不降低系统安全设置来绕过检查。
- 原生减少动画偏好启用后复核静止状态，再恢复；结束时正常退出测试实例，检查进程、热键、托盘、实例锁和正式数据指纹。

最新结论见[2026-11-13 本机验收](baselines/2026-11-07-slint-rs13/completion-2026-11-13/README.md)：当前exe物理矩阵、跨屏、录制脉冲与外观取舍已补齐，RS-13已验证本机可测范围。此前的[当前构建续验](baselines/2026-11-07-slint-rs13/completion-2026-11-12/current-build-followup/README.md)、[自主原生续验](baselines/2026-11-07-slint-rs13/completion-2026-11-12/autonomous-followup/README.md)及[早期布局/物理复核](baselines/2026-11-07-slint-rs13/completion-2026-11-11/layout-followup/README.md)仍保留原身份及失败样本。200%不可选、其他系统/驱动/IME未测，不宣称所有原生帧或精确光子时序一致；默认renderer与G4继续留给RS-14。
