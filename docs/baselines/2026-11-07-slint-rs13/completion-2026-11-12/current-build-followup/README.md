# RS-13 当前构建续验

2026-11-12（北京时间）。本轮继续执行无需用户协助的 RS-13 项目，**RS-13 保持进行中**。当前两个产品的各 206 项输入及完整验证回执匹配；新增 104 张当前源码静态图、26 段组件录像和原生诊断。没有重建或替换产品 exe，只有独立截图/动画夹具重新编译。

Software 产品 SHA-256 为 `5E7C017DDA3FF691E7FF8DCBFEAF149E5A766C1F8DC73A816BB5C4A9F7C4C66B`，FemtoVG 为 `D81F31ED8AB3C0D45A3565CDC8AA412B3FB870C0CC350204178CDE44456B6487`。此前 48 段原生录像属于各 157 项输入的旧构建，继续保持原身份；[源码差异与回执](build-identities.json)记录共享输入中七项变化，包括版本显示和录制动作布局。

## 当前源码的组件验证

- [静态图册](static/gallery.html)有 104 张原始 PNG、52 对深浅场景，覆盖首页、空分组、列表、网格、长名称、导入、七个设置页、滚动底部、错误/更新/扫描及录制状态。两张总览已检查，录制动作的文案/位置与当前源码相符。
- Wails 独立 DOM 参照下，[Software](layout-software.json)和 [FemtoVG](layout-femtovg.json)各 66 项核心矩形满足 ≤2 逻辑像素；[12 项标题原点回归](heading-regression.json)通过。这个结论只覆盖指定边界，不代表字体或原生窗口像素完全一致。
- [26 段组件动画](component-motion/verification.json)共 936 张源帧，隐藏/减少动画静止、有限动画终态、首次过渡帧可交互、两行同列入场节奏通过。这是当前 UI 源码的专用夹具结果，不计入产品原生时序或物理输入。

## 原生过渡与分析工具修正

当前两个产品分别取得设置进入/退出的源帧，活跃片段为 Software 18/9 帧、FemtoVG 17/18 帧，内部最大间隔约 33.37ms。但是首个中间态前的源样本间隔分别约 500.50ms、144994.94ms、350.35ms、483.82ms；这些间隔不能作为已观察到的触发时刻。

本轮修复 `verify-rs13-transition-contract.py`：从原始样本重算进入、过渡内部和结束边界的间隔，不再只相信汇总字段，也不再忽略很早缓存的静态基线。[21 项校准](tools/transition-calibration.json)覆盖已知波形、错误时长/easing、退出滑动、过期基线、缺少起止帧、内部漏帧、错误元数据及无效数值；[4 项 ROI 防误判](tools/roi-guard-checks.json)也通过。

修正后的 [Software](transitions/software-contract-with-boundary-gate.json) / [FemtoVG](transitions/femtovg-contract-with-boundary-gate.json)报告将四段全部记为 **insufficient（样本不足）**。允许自由拟合触发时，它们可以兼容声明波形，但这不能签收时长或 easing；截掉首个中间态、把新首帧强制当作透明度零的派生拟合也不能用于产品结论。退出标题可见位移约 0.25/0.5px，仅作为观察。

完整原生追踪、派生切片及原帧 SHA 映射均保留。Software 的三份父追踪在后续等待中触及每片段 128MiB 存储上限；FemtoVG 设置过渡和 Software 第二次会话未超限。五个采集器的 GPU/写队列拒绝均为零，不能把“队列零拒绝”写成“全部片段未超限”。

## 当前录制控件

Software 当前产品实际打开录制态，合成 Esc 后恢复原 `Alt + Space`，没有保存设置。其限额前 85 张原帧中，除去缓存基线有连续 84 帧、1.668 秒的活动段，最大源间隔 33.367ms；固定控件 ROI 有 19 次有效变化。[局部序列](pulse/pulse-roi-active-timeseries.json)中的高位区间起点为 183.518/1368.027ms，相隔 **1184.509ms**，中间也观察到低位区间，与声明的约 1.2 秒脉冲相符。[像素放大图](pulse/pulse-roi-active-timeline.png)保留原时间标签；不据此宣称精确 easing、幅度或显示器呈现时刻。

FemtoVG 当前产品的录制补验中，桌面工具在重新选择窗口后仍返回与目标不符的旧画面；没有在该画面上发送输入。实例已用隔离测试通道正常退出，该次没有新增原生脉冲通过记录。当前源码的 FemtoVG 录制组件录像已完成，和原生待验项分开记录。

## 清理、证据与剩余范围

[清理检查](cleanup.json)确认六份产品、五份采集器正常退出，无任务实例端点/进程，Alt+Space 可以注册且立即释放。六份隔离历史保持 seed SHA，未发现启动回调。正式五项文件与本轮之前已有的参考快照一致；没有虚构一份本轮开始前即时采集的快照。未改显示设置、正式数据、安装程序或默认 renderer，未提交、推送或发布。

原生帧通过选定 HWND 的 Windows Graphics Capture 取得，实际 96 DPI。所有窗口输入为合成刺激，不能代替真实微软拼音候选/翻页/选词/Esc、中英输入、物理 125/150/175% 或跨屏；此前用户反馈保持各自原 exe 身份，200% 仍为本机不可选。字体/阴影/系统菜单等外观取舍、精确原生过渡和当前构建物理项仍待，默认 renderer/G4 未放行。

[验证摘要](verification-summary.json)、[证据审计](evidence-audit.json)、[会话记录](sessions.json)、[工具来源](tools/identities.json)及 [公开校验和](SHA256SUMS.public.txt)可复查。审计区分完整性通过和产品验收：147 张原生源/派生帧、104 张静态图、936 张组件帧的 SHA 已核对，四个原生设置过渡仍为样本不足。
