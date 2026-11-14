# RS-13 布局、原生动画与物理输入续验

2026-11-11（北京时间）。本轮修复此前 Wails 对照中的主体布局、控件及图标差异，完成两种最终构建的完整自动回归、组件矩形比较、原生动画采集和本机可选缩放的微软拼音复核。**RS-13 保持进行中**：132 项核心矩形比较通过，不等于全部原生像素与动画轨迹通过；精确原生时长/曲线、下列视觉细节及本机不可选的 200% 仍有明确边界。默认 renderer 与 G4 未决定。

## 已修复与验证

- 设置恢复 82px 页头、82–132px 页签、24px 正文内边距和 68px 底部操作区；数字框竖排、150×37，取消/保存 58×37 / 90×37。加入主题预览、复选框和带说明的排列/主题下拉，七个页签、草稿、保存与重启保留。
- 首页分组/行距、列表选中与固定按钮、六列网格的 38px 图标、自定义入口、底部键盘提示及系统快捷的 12 项折叠/展开接通。多行入场按列延迟，隐藏和减少动画的组件语义保留。
- 自定义页恢复固定说明/添加区、独立长列表和固定页脚；多文件导入预览仍可逐项查看。可见项图标复用原有后台请求与有界 LRU，隐藏后释放模型引用。
- Windows Shell 不接受部分 `\\?\` 规范路径拼写，现只在图标 FFI 边界转换路径，ID、持久化路径与缓存签名不变。原生回归比较普通/规范路径的真实图标，页面回归核对可见自定义图标和系统快捷展开。
- 下拉加入进入/离开过渡，录制加入脉冲轮廓；Software 的旋转继续使用原 Fluent Bézier 控制点，没有引入位图序列或运行时依赖。

[最终身份](final-build-identities.json)与[当前身份复核](current-identity-audit.json)记录 Software `A64BE7CCBA0F187EA89EA0C1933658DFE67BC440825EB29D73DDF2C430848659`、FemtoVG `8C5A183653EB678E68B81091EDB3428628EE059ED3CA746BE8453EFA5291D547`。两份各 157 项构建输入逐项匹配；各 **80 项 Rust 测试、22 项输入、7 项业务、33 项页面/重启断言**及完整检查、Release、原生/数据兼容与 `-SmokeTest` 通过。原生页探针处理延迟 changed-handler、正常隐藏/恢复和滚动后的可见项；曾失败的前台、滚动阈值及图标样本保留在[验证历史](verification-history.json)。此前 Go 1.25.6 旧版回归仍见[首轮验证](../verification.json)，本轮不据文档整理重复构建产品。

## 布局与原生页面

[独立 Wails DOM](reference/settings-geometry.json)来自未改动的前端，固定 640×660 CSS 视口与合成接口，[资源指纹](reference/frontend-identities.json)可复查。最终 [Software](layout-final-software.json) / [FemtoVG](layout-final-femtovg.json) 各 **66 项实际组件矩形**在 ≤2 逻辑像素内，共 132 项；包括七页的头部、正文、操作按钮及适用的数字框、下拉和目录框。该检查不量取字体、阴影、全部首页控件，也不以 DOM 冒充物理窗口。

[最终静态图册](static/gallery.html)库存为 104 张原始 PNG、52 对同源场景；[12 项标题像素回归](heading-final.json)通过。**四张 `settings-6-bottom` 实际为折叠的关于页**，原因是夹具设定展开后被延迟页签回调复位；原图保留，[更正记录](static-coverage-corrections.json)优先于图册自动计数，按名称有效的场景为 100 张。它们不再证明许可底部覆盖。

最终原生截图另存：[浅色首页/常规/自定义对照](native-layout-final.png)、[深色对照](native-layout-dark.png)、[深色六列与跨行导航](native-grid-dark.png)。每张原生窗图保持 640px 原宽，短窗口只补空白，不缩放后宣称容差。三候选新增 **47 张公开深色原始 JPEG**覆盖首页、七设置页、排列下拉、网格、导航和自定义页；具体身份与排除的本机扫描提示见[页面清单](native-pages/observations.json)。Software 实际滚到第 999 项，头部/底部仍固定；不存在目标的图标使用占位，不伪造文件图标。

最终 Software 的[浅色许可底部](native-final/software-license/about-license-bottom-0.jpg)和[内层文本滚动](native-final/software-license/about-license-inner-scroll-0.jpg)、最终 FemtoVG 的[深色 AboutSlint](native-pages/femtovg/settings-6-about-slint-0.jpg)补充真实许可可达性。外层正文与内层许可文本分别滚动，保存/取消保持可达；没有为修正夹具计数而替换旧原图。

## 物理 DPI 与微软拼音

[原始物理矩阵](physical-input.json)保留各自原 exe SHA、PID/创建时间、HWND DPI、编辑计数及用户原话。两种 renderer 在本机可选 **100/125/150/175%（96/120/144/168 DPI）**均完成真实键盘复核，候选定位/选词/取消反馈正常；FemtoVG 同组另明确确认翻页与英文 `abc`。Software 150% 的位置/选词/取消是用户随后回忆确认，不将它写成当时直接拍到的候选像素。

Software 100% 有候选框原图，其余档位结合实际 DPI/中文显示与用户反馈；编辑事件本身不能证明候选框每项行为。175% 的 640×660 逻辑目标受工作区限制，窗口高度约压至 569 逻辑像素，滚动及操作区仍可达。用户报告最大可选缩放为 **175%，200% 未覆盖**；没有写注册表或用 `SLINT_SCALE_FACTOR` 代替物理 DPI。用户已确认恢复 100%，最终日志为 96 DPI。

原矩阵对应 Software `5EB0E03B…` / FemtoVG `C55E36CB…`；后续图标/下拉等变动有独立[源码差异](physical-to-final-source-delta.json)，`input.rs`、`lifecycle.rs`、`main.rs`、`geometry.rs`、`hotkey.rs` 保持一致。最终 exe 的 100% 单次补验见[最终物理记录](final-physical-input.json)：用户确认 Software 全部正常，FemtoVG 正常且跨屏输入正常。Software 最后会话有 31 次接收、9 次合成记录；FemtoVG 最后会话没有匹配的合成序列/跨屏几何记录，**最终 FemtoVG 的这两项采用用户报告，不声称已得到仪器完整佐证**。两者正常退出。Computer Use 也可能产出 `synthetic=false`，该标记不单独证明物理操作。

## 原生动画与成本观察

[原生动画复核清单](native-motion-review.json)包含 14 类：设置/自定义进入与退出、列表进入/选择、首页进入、tile hover/按压、齿轮、扫描、录制、下拉进入/退出。Wails 17、原布局 Software 15 / FemtoVG 6、最终 Software 6 / FemtoVG 14，共 **58 个公开片段**；其中 4 次受限/延迟尝试排除，原件仍可追溯。最终 Software 六段补录覆盖之后修改的图标/面板、下拉与脉冲；其他片段保留原身份。

可直接比较 [Wails 设置进入](native-motion/wails/settings-enter-retry.webp)、[最终 Software](native-motion/software-final/settings-enter.webp)、[最终 FemtoVG](native-motion/femtovg-final/settings-enter.webp)，以及 [Wails 扫描](native-motion/wails/scanning.webp) / [FemtoVG 扫描](native-motion/femtovg-final/scanning.webp)、[Wails 下拉](native-motion/wails/dropdown-enter.webp) / [Software 下拉](native-motion/software-final/dropdown-enter.webp)。每份 `recordings.json` 保存原始帧 SHA、时钟、限制与派生范围，无损 WebP 使用实际观察间隔；仅裁掉空闲前后段、尺寸变化时补边，接触图是缩略总览。

采集器固定 PID/创建时间，通过 Windows Graphics Capture 只读取选定 HWND 的客户区内侧，保存 WGC 源时间及观察 QPC，关闭鼠标捕获；这不是最终桌面/显示器光子时间。原生片段能核对状态变化与终态，**还不能严格签收全部 100–350ms 缓动轨迹**：例如 Wails 设置进入有效重采的保存帧间隔最高约 296ms，跨过了需要核对的 220/280ms 过渡。没有把稀疏片段的总长当作动画时长或 60Hz 证明。

[26 段组件检查](component-motion/verification.json)对应 936 张源帧，进入/离开、两行同列节奏、即时可交互、扫描/录制动态、有限动画结束与减少动画/隐藏静止均通过。组件采样没有原生帧呈现保证，不能补造原生缺帧。[原阶段 CPU 括号](native-phase-observations.json)与[最终 20 段 CPU 括号](final-native-cost-observations.json)包含截图、操作和空闲尾部，不能据此排名 renderer、制造精确动画 GPU 成本，亦未重标 RS-12 冻结程序的常驻预算。

## 仍待验证的具体项目

- 原生全动画的精确时长/轨迹仍需更密集且低干扰的刺激/帧同步采集；现有 14 类录像是完整的类别采集，不是全部曲线验收。
- 视觉细节仍有差异：快捷键是组合文本而非 Wails 的分键帽，网格底部未显示左右方向键提示，导入区边线是实线而非虚线；许可区合并多项文本并增加真实 AboutSlint，与旧版分块不同。许可展开三角在本机字体下有缺字迹象，Software 深色许可滚动的一张截图出现版本行重影，尚未隔离为稳定产品缺陷或采集/重绘问题。这些不包含在 132 项矩形通过结论内。
- Windows 原生菜单/选择器跟随系统，和 Wails 的自绘菜单样式不同；缺失目标的目录/管理员项在 Slint 禁用。OS 字体、渐变/阴影/抗锯齿差异未按原生无损像素全量签收。未为截图执行关机、清空回收站、安装或 UAC 等动作；已有功能权限验收保持各自 RS-11 身份。
- 200% 为设备配置限制；本轮没有新增全部确认提示、Windows 10、第三方 IME 或所有驱动覆盖。当前构建的最终 FemtoVG 物理反馈缺少配套合成/跨屏日志，来源限制如上。

本轮不将上述项目默认为已接受的取舍，不选择默认 renderer，不放行 RS-14/G4。下一轮可直接针对这些具体项推进，无需重复已完成的本机缩放矩阵。

## 失败保留与清理

DXGI 采集失效、WGC 客户区与窗口尺寸差 1px 导致零有效帧、前台激活失败及受限片段均保留。旧 WGC 曾在已写 `end` 后 teardown 阻塞，仅终止该只读观察器；目标产品正常退出，见[历史会话](historical-session-status.json)。最终 v4 观察器落盘后正常结束，未把观察器清理改标为产品退出失败，也未给重启/中断缺失 completion 的会话补造退出码。

[工具拒绝检查](native-tool-checks.json)覆盖错误进程身份和零帧，派生工具有[指纹](tool-identities.json)。[最终清理](cleanup.json)确认测试程序/采集器/隔离 WebView 为零、实例端点消失、Alt+Space 可重新注册且立即释放，五项正式文件状态与保存的前后哈希括号一致。测试和 seed 全部隔离；未提交、推送、安装、发布或替换正式程序。

复现见[RS-13 操作步骤](../../../../rust-slint-rs13-testing.md)。本目录原件/派生件校验和见 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)，最终证据审计见 [evidence-audit.json](evidence-audit.json)。
