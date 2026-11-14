# RS-13 视觉细节与动画时序续验

后续已恢复工具并完成三候选原生补采，见[自主续验](autonomous-followup/README.md)。下文保留当时 Escape 中断及该轮未完成状态，后续成功未覆盖原件。

2026-11-12（北京时间）。本轮修复键帽、网格提示、导入边线、许可展示及面板/下拉时序；两种新构建的完整回归、104 张有效静态场景与 26 段组件录像通过。**RS-13 仍为进行中**：原生补录过程中 Computer Use 两次返回“物理 Escape 停止”，第二次发生在用户明确恢复后的首次窗口选择，未能完成三候选的密集原生复核。该工具状态不能区分第二次按键与停止状态延续，未擅自归因。

## 已完成的修复

- 快捷键改为独立键帽，保留录制提示、真实组合值和保存语义；生产入口只新增一个将组合文本拆成显示模型的纯回调。
- 六列网格页脚补齐左右键提示，Enter 使用已有 Fluent 矢量图标，状态文本相应预留宽度。
- 导入区改为逻辑像素矢量虚线；首版缺失显式原点导致边线出现在中间，已修正并通过新静态图检查，原尝试保留。
- Fluent/go-pinyin/Go Unicode 许可分块、各自内层滚动，保留真实 AboutSlint；展开符号改为矢量路径，支持点击、键盘和辅助功能操作。AboutSlint 独立裁剪，新图未再出现此前的版本行重影；原生运行时还需复核。
- 面板离开按 Wails CSS 改为 **100ms ease 仅淡出**，不再退出时下移；下拉透明度 **140ms ease** 与位移/尺寸 **180ms 主缓动**分离。设置进入仍为 220ms 透明度/280ms 位移。
- 静态夹具在页签延迟回调之后设置展开和滚动，再截图，并断言许可区确实展开且滚动。此前四张误标的许可底部图未被改写；本轮 104 张库存中对应四张已取得有效状态。

## 验证与身份

[最终构建身份](build-identities.json)包含完整 exe SHA、工具链、各 157 项输入及验证回执。两 renderer 各 **80 项 Rust 测试、22 项输入、7 项业务、33 项页面/重启断言**以及格式、clippy、数据双向兼容、NTFS 拒绝写入、原生功能、后台启动和 Release 通过；最终入口为 `verify-rust.ps1 -SmokeTest`，见[验证历史](verification.json)。v1/v2 是修复中的早先快照，v3 才对应最终产品输入。

[静态图册](static/gallery.html)为 **104 张、52 对**深浅场景；[12 项标题回归](heading-regression.json)通过。实际矩形对照 [Software](layout-software.json) / [FemtoVG](layout-femtovg.json) 各 66 项满足 ≤2 逻辑像素，参照仍是[上一轮独立 Wails DOM](../completion-2026-11-11/layout-followup/reference/settings-geometry.json)。这些是明确控件的逻辑边界，不声称全原生像素一致。

可检查[键帽](static/software/light-settings-1.png)、[网格提示](static/software/light-grid.png)、[导入虚线](static/software/light-custom.png)、[Software 许可底部](static/software/light-settings-6-bottom.png)和 [FemtoVG 许可底部](static/femtovg/dark-settings-6-bottom.png)。

[26 段组件验证](component-motion/verification.json)共 **936 张源帧**，有限动效结束、隐藏/减少动画静止、即时可交互与两行同列节奏通过。两次 FemtoVG 编码队列满分别在扫描和 reduced-hover 阶段中止；本轮最终使用**相同 exe、相同容量 2 队列**在没有其他采集时重试，完整 468 帧通过，Software 成功记录原样复用。一次静态 FemtoVG 仅采 44 张后退出 1，原包装器未保存 stderr，原因保持未知；同一 exe 串行重试取得 52 张，源码指纹前后匹配。[失败与重采](capture-attempts.json)分别保存，未通过扩大队列或修改旧原件掩盖失败。

## 原生时序补验的实际范围

观察器 v5 改为复用读回纹理、128MiB 有界写帧队列，保存重复帧及原 WGC 时钟。Wails 设置进入/退出的有效片段最多约 66.7ms 源帧间隔，写队列拒绝为 0，无存储超限；设置进入只有三张中间态，仍不足以严格签收曲线。可查看[原帧清单与预览](native/wails-v5-previews/recordings.json)、[设置进入](native/wails-v5-previews/settings-enter-1.webp)、[设置退出](native/wails-v5-previews/settings-exit-1.webp)。`settings-enter-2` 在 Escape 停止时中断，**明确排除**，发布预览不代表验收。

[标题光学估计](native/wails-v5-title.json)将面板标题与底层首页分开，拟合透明度/垂直位移；[已知 220/100ms 序列校准](panel-calibration.json)和[输入完整性拒绝检查](analysis-guard-checks.json)通过。但原生退出估计范围仍较宽，不能据标量拟合冒充独立曲线或 GPU 执行时长。

v6 进一步使用三个非阻塞 GPU 读回槽，队列溢出明确计数；MSVC `/W4 /WX` 构建通过。用户确认第一次 Escape 是误触并授权继续后，重新打开 Wails；**首次窗口选择又被 Computer Use 以物理 Escape 停止拒绝，没有产生任何新 UI 输入和命名片段**。v6 只有构建/启动及正常退出证据，没有原生动画验收，见[原生状态](native-status.json)。测试结束通过正常 API 请求退出，产品与观察器退出码均为 0。

## 物理输入与剩余项目

前一轮 [100/125/150/175% 微软拼音矩阵与最终用户确认](../completion-2026-11-11/layout-followup/final-physical-input.json)保持原身份；本轮不要求重做完整矩阵。`input.rs`、`lifecycle.rs`、原生 DPI 几何及热键实现未变，[源码差异](physical-source-delta.json)列明本轮主入口的显示回调及其他变化。**本轮新 exe 没有新增物理 IME 通过声明**；200% 仍是设备不可选条件。

下一项为三候选的密集原生设置/导入/下拉及修复细节复核；待桌面工具恢复可操作后续采。此前原生菜单/字体/阴影等差异继续保留，未以组件图册代替原生签收。RS-13、默认 renderer 与 G4 均未提前放行。

[清理检查](cleanup.json)确认无任务测试进程/采集器/隔离 WebView、实例端点消失、Alt+Space 可重新注册并释放，五项正式数据状态与本轮保存的哈希括号一致。源码、失败原件与旧证据保留；未提交、推送、安装或发布。

本目录完整性见[证据审计](evidence-audit.json)与 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)，采集工具来源见[工具指纹](tool-identities.json)。
