# RS-13 本机视觉与实机验收

2026-11-13（北京时间）。**RS-13 已验证，适用范围为本机 Windows 11 x64、100/125/150/175% 与已测跨屏。** 本轮补齐当前两个 exe 的真实微软拼音、DPI/跨屏、FemtoVG 原生录制脉冲和两 renderer 的原生设置进入/退出证据；用户接受了列明的外观差异。200% 当前不可选，不计通过。默认渲染器、G4、正式安装或发行仍归 RS-14，本轮没有放行这些事项。

## 构建与已有视觉证据

两份产品未重建，Software SHA-256 为 `5E7C017DDA3FF691E7FF8DCBFEAF149E5A766C1F8DC73A816BB5C4A9F7C4C66B`，FemtoVG 为 `D81F31ED8AB3C0D45A3565CDC8AA412B3FB870C0CC350204178CDE44456B6487`。[构建身份](build-identities.json)逐项验证各 206 项当前输入与完整检查/冒烟回执，保持此前各 80 项 Rust 测试和 22/7/33 项输入/业务/页面断言的原范围。

当前源码的[104 张静态图、26 段组件动画/936 帧、132 项核心矩形和 12 项标题回归](../completion-2026-11-12/current-build-followup/README.md)有效。三候选各 14 类/48 段原生动画与 43 张页面仍见[自主原生续验](../completion-2026-11-12/autonomous-followup/README.md)；它们保持当时 157 项输入的原 exe 身份，不能改标为当前程序。后续变化通过当前静态图、录制控件和设置过渡补验覆盖，样式模块指纹对应关系见[对照来源](appearance/sources.json)。

## 当前 exe 的物理输入与缩放

[八项新物理矩阵](physical-input.json)均有本轮用户直接反馈及对应 PID/创建时间/exe SHA，不复用早先物理结论。两 renderer 的 100/125/150/175% 分别对应 96/120/144/168 DPI。每档用户完成候选定位、翻页、选词、合成时 Esc 取消、英文 `abc` 及布局检查；175% 还确认工作区约束与滚动正常。日志只留合成状态、焦点、可见性及字节数，不保存查询原文。

Software 四档由独立原生快照确认。FemtoVG 的 100/125/175% 同样匹配；150% 的稍后快照读到 96 DPI，因为用户在该快照之前已改回 100%。[原快照不匹配](physical/femtovg-150-physical.json)照旧保留，另存[实际 144 DPI 输入区间](physical/femtovg-150-validated-log-interval.json)：行 199–261 内有三个合成和三个非合成编辑，输入发生在 960×315/882/990 的 144 DPI 布局之间，之后才出现 96 DPI 的显示变化。没有为此要求用户重做或重标原快照。

[两份跨屏结果](cross-screen.json)均由用户物理拖动并现场输入确认，原生观察器与程序日志记录了 168/96 DPI 及位置变化。用户确认最后恢复原来的 100%，最终原生快照为 96 DPI。原[旧矩阵](../completion-2026-11-11/layout-followup/physical-input.json)与[此前最终反馈](../completion-2026-11-11/layout-followup/final-physical-input.json)原件保持不变。

这些日志中的 `synthetic=false` 单独不能证明真实输入或候选行为；物理来源以[逐条用户回复](operator-reports.json)为依据。程序日志与被动窗口采样补充实际 DPI、合成和焦点状态，不能独立观测候选翻页。

## 原生录制脉冲

当前 FemtoVG 的[原生控件记录](native/pulse/motion-roi.json)覆盖 153 张连续活动帧、约 2.703 秒，最大源间隔 33.369ms；固定 ROI 有 143 次变化。三个 900ms 短片在相同源时间上去除字节一致的缓存边界重复帧，[源映射](native/pulse/source-manifest.json)保留每帧 SHA，像素及时间均未改写。

[局部脉冲序列](native/pulse/timeseries.json)的两个峰位于 483.810/1685.014ms，相隔 **1201.204ms**，与 1.2 秒声明在采样分辨率内相符；可查[像素放大图](native/pulse/timeline.png)和[原时钟预览](native/pulse/preview/recording.webp)。用户报告“蓝点脉冲正常，Esc 已恢复原快捷键”，日志确认录制关闭；没有保存设置。Software 同身份的[此前脉冲样本](../completion-2026-11-12/current-build-followup/README.md)继续保留，其父追踪的超限不改写为未超限。

## 原生设置过渡

[全部 18 个当前候选区间](native/combined-transition-results.json)中，7 个满足未修改的固定波形/采样门槛，1 个误差超限，10 个因采样不足未通过。合格样本覆盖 Software 进入一次、FemtoVG 进入两次，以及两个 renderer 各两次退出。保留所有候选，不能用通过样本覆盖原失败。

- Software 进入的透明度 RMSE 为 0.0404、位移 RMSE 为 0.490px；FemtoVG 两次进入分别为 0.0344/0.0027 和 0.609/0.239px。与独立 Wails CSS 的 220ms 透明度、280ms 位移/8px 声明相容。
- 两种 renderer 的退出在预先固定的 350/500ms 等待后通过：Software 透明度 RMSE 0.0317/0.0366，FemtoVG 为 0.0417/0.0453；触发前源样本间隔约 33.37ms，位移 RMSE 小于 0.18px。符合 100ms `ease`、只淡出并恢复正常终态的观察。
- 较早 Software 退出样本的 RMSE 为 **0.0558**，高于固定 0.05 门槛；它仍为 `mismatch`，另一次为 66.7ms 起点空隙导致 `insufficient`。后续两次相同 exe、预先固定等待时长的复测通过，没有改产品、队列容量或阈值。该单样本偏差不由后续通过抹去，也不宣称所有动画帧完全一致。

退出自动补采先让现有录制脉冲产生新原生帧，再由已有 opt-in stdio 调用正常 `home/settings` 回调；进入先借现有扫描刷新。改变的是测试刺激时序，产品源码与动画参数未动。合成补采和物理输入矩阵分为不同实例及目录。原始 WGC 时钟、帧 SHA、边界检查与失败均保留；[21 项分析校准](autonomous-followup/transition-calibration-final.json)通过。小标题区域的[独立模型诊断](autonomous-followup/panel-calibration.json)仅作诊断，没有取代原生门槛或原结果。

“相容”表示在源帧/像素容差内支持声明的动画行为，不能证明唯一 easing、GPU 逐帧执行耗时或显示器光子时刻。计划 §6.2 的本机逐帧视觉/时长审查按上述范围交付；精确光子时序没有被新增为已通过项。

## 外观取舍、清理与边界

用户查看[不缩放的原生对照](appearance/review.png)及菜单示例后明确回复“接受这些外观差异”。[接受范围](appearance/acceptance.json)只覆盖文字抗锯齿/细小字重、阴影/光晕强度，以及 Windows 原生菜单/文件选择器的系统样式；不授权未列明的交互变化、默认 renderer 或发行。

[最终清理](cleanup-final.json)确认六份产品正常退出，无任务进程/实例端点，Alt+Space 临时注册后立即释放，五项正式文件均匹配本轮开始前的新快照。两个物理实例以及四个后续原生测试实例的配置/历史未变；没有启动测试候选应用。11 次完整采集正常退出、GPU/写队列零拒绝且均未触及存储限额。两类未完成尝试照原样保留：隐藏窗口导致 WGC 拒绝、自动触发等待超时；另一个固定时段仅录到首页，未计设置动画通过。

200% 当前显示配置不可选，Windows 10、其他驱动和第三方 IME 不在本机范围。默认 renderer、G4、正式系统安装/升级和发布仍未放行。未提交、推送、安装、发布或替换正式程序。

[最终验收范围](final-verification.json)、[证据审计](evidence-audit.json)、[公开校验和](SHA256SUMS.public.txt)及[任务实例记录](sessions.json)支持复查。较早的 [verification.json](verification.json)保留用户操作刚结束时的中间检查点，后续新增的合成过渡结果以 `final-verification.json` 为准。
