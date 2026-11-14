# RS-13 自主续验：原生动画与最终页面

2026-11-12（北京时间）。用户确认此前 Computer Use 中断是误操作，并说明后续无法协助。本轮重置工具会话后恢复正常，**完成当前三份程序各 14 类原生动画，共 48 段，以及最终 Slint 深浅关键页面的自主复核**。没有要求用户再次操作，也没有重建产品或重标此前物理输入的 exe 身份。

## 本轮结果

- Wails、Software、FemtoVG 各录制设置进入/退出两次，自定义页进入/退出、下拉进入/退出、录制脉冲、列表进入/选中、首页进入、tile 悬停/按压、齿轮悬停及扫描。每候选 16 段，覆盖 14 类；[覆盖与原时钟](motion/coverage.json)明确区分重复样本。
- 六份采集的 GPU 读回队列拒绝、写队列拒绝均为 **0**，无存储超限；七份产品实例及六份观察器均正常退出。采用与上一轮相同的 v6 exe，原生窗口独立、串行运行，无编译负载和进程树高频采样。
- [48 项局部控件区域检查](roi-summary.json)均观察到动态变化。ROI 排除搜索光标和大部分工具提示，避免以其他区域的变化证明旋转/脉冲；[拒绝与假阳性检查](roi-guard-checks.json)覆盖错误进程、未校准 DPI、仅光标闪烁和真实局部变化。
- 最终 Software/FemtoVG 的键帽、虚线边线、主题下拉、录制/Esc 取消、许可展开符号及分块、AboutSlint、固定操作区和网格四向导航已在原生窗检查。两 renderer 的深色键帽/许可也复核；**本轮未复现缺字和版本行重影**。原始 [43 张 JPEG 清单](pages/observations.json)保留截图身份，界面中的蓝色指针强调来自 Computer Use。
- 所有输入均为自动化合成刺激，tile 按压采用移出控件后释放。七份历史文件 SHA 与各自 seed 一致，未发现启动回调，[会话结果](sessions.json)保留 PID/创建时间、exe SHA 和退出回执。

[原生细节对照](native-details.png)使用 Wails/Software/FemtoVG 原始 640×660 窗图，不缩放单张截图。可单独查看 [Software 网格](pages/software-motion/grid-0.jpg)、[FemtoVG 深色网格](pages/femtovg-motion/dark-grid-0.jpg)、[Software 深色许可](pages/software-dark/about-slint-0.jpg)、[FemtoVG 深色许可](pages/femtovg-motion/dark-about-slint-0.jpg)。

## 动画时序与测量边界

原始帧通过选定 HWND 的 Windows Graphics Capture 取得，保存源帧时间、观察 QPC、实际 96 DPI、客户区映射及 12px 内侧裁剪；不捕获整个桌面或鼠标。无损 WebP 按 **WGC 源时间**回放，只移除前后空闲、在尺寸变化时补边，末帧延长 200ms 便于查看。`recordings.json` 保留每张源帧 SHA 和完整时间；接触图仅作缩略浏览。

对照示例：[Wails 设置进入](motion/wails-motion/settings-enter-1.webp)、[Software 设置进入](motion/software-motion/settings-enter-1.webp)、[FemtoVG 设置进入](motion/femtovg-motion/settings-enter-1.webp)；[Wails 下拉](motion/wails-motion/dropdown-enter.webp)、[Software 下拉](motion/software-motion/dropdown-enter.webp)、[FemtoVG 下拉](motion/femtovg-motion/dropdown-enter.webp)。其他片段均在[覆盖清单](motion/coverage.json)直接链接。

[标题光学测量](motion/panel-summary.json)使用固定标题区域，独立估计透明度和垂直位移；延续上一轮已通过的已知 220/100ms 合成校准。设置进入/退出的有效过渡区源帧间隔最高约 **16.7–50.1ms**，每段有 4–14 张中间态。两次透明度时长点估计分别为：Wails 进入 244/254ms、退出 120/112ms；Software 进入 214/210ms、退出 106/126ms；FemtoVG 进入 244/294ms、退出 122/116ms。可见退出标题的最大垂直偏移为 0–0.5px，符合退出只淡出的观察。

这些点估计与 220/100ms 源码时长接近或落在其拟合可辨识区间，但部分区间仍很宽；例如 FemtoVG 第二次进入的位移拟合为 372ms，误差约 0.31px，**不能据此断言实际位移动画从 280ms 改成 372ms，也不能据其他点估计宣称精确曲线已签收**。像素量化、字体/透明度、帧采样及合成会影响可辨识性，原始样本与误差全部保留。当前能确认过渡方向、有限终态和退出无明显滑动；独立属性的精确曲线及最终显示器呈现时间仍未严格证明。

扫描时长日志补充说明：Software 约 **1.058s**，FemtoVG 约 **1.114s**，本轮原生扫描片段已覆盖一秒旋转周期；开始时依据静态截图作出的“不足一圈”临时判断由该日志更正。录制脉冲也保留超过 1.2s 的连续片段，局部变化已核对。周期与轨迹的组件逐帧验证仍见[最终 26 段组件录像](../component-motion/verification.json)，组件结果不冒充显示器帧时间。

## 构建、差异与剩余范围

[构建身份](build-identities.json)核对当前两个产品 exe、完整检查/冒烟回执及各 **157 项输入**，与上一轮最终构建一致。继续使用已通过的各 80 项 Rust 测试和 22/7/33 项输入/业务/页面断言，以及 132 项核心矩形、12 项标题回归、104 张有效静态场景和 26 段组件录像；本轮仅新增测量工具、原生证据与文档，未为读取证据重复构建产品。

原生仍可见少量样式差异：字体抗锯齿、阴影/渐变、下拉阴影，录制动作的文案/脉冲位置，以及系统原生菜单/选择器与 Wails 自绘菜单的外观。许可中增加 Go Unicode 声明与真实 AboutSlint，保留重构工程身份。上述差异没有被纳入 132 项核心矩形通过结论，亦未默认为已获视觉取舍签收。

**本机可独立执行的本轮检查已完成，RS-13 继续保持进行中。** 严格独立原生曲线与少量外观取舍尚待验收；当前新 exe 的真实键盘/微软拼音候选、物理缩放与实际跨屏仍不由本轮合成输入替代。前一轮[物理矩阵](../../completion-2026-11-11/layout-followup/physical-input.json)和[最终用户反馈](../../completion-2026-11-11/layout-followup/final-physical-input.json)保留原 exe 身份，200% 仍为本机不可选条件。用户暂时无法协助，这些项目留待后续，不再请求本轮参与。

本轮没有执行系统危险动作、UAC、安装或发布；不新增性能排名或默认 renderer/G4 结论。此前 CPU/内存记录保持各自原身份，当前帧采集没有启用进程树采样，不能用它推导新的动画成本。

[清理](cleanup.json)确认测试进程、采集器、隔离 WebView 和实例端点均已退出，Alt+Space 可注册且立即释放，五项正式文件状态与本轮哈希括号一致。[证据审计](evidence-audit.json)、[工具指纹](tool-identities.json)及 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)可复查；此前两次 Escape 中断原件继续保留，本轮成功记录没有覆盖它们。
