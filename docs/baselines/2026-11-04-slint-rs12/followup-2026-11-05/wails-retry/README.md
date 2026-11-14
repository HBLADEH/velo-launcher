# Wails 桌面对照续验与采集校验修正

2026-11-05/06（北京时间）。新建独立数据和已退出 WebView profile 缓存副本，复用原输入测量 exe，SHA-256 `14FB81E432B0543A32DC5E7B1698596A15BB5824136BA045C0AFBB36201B7AFA`。没有替换正式程序、配置或登录项。严格冷启动依用户安排继续暂缓。

2026-11-06 最终 Wails DPI 续验同样复用隔离 `latencyprobe` 输入测量构建，不等同官方 Release。用户完成右屏拖动、100%→150%→恢复 100% 后报告“都正常完成了布局输入都正常”。原生被动采集确认右屏转换：640×588 @96 DPI → 960×882 @144 DPI → 640×588 @96 DPI，并观察到用户在 150% 隐藏时的状态；Windows 显示 probe 前后配置一致。

该 Wails `Computer Use` 接口没有返回可操作窗口对象，本轮没有 Wails 像素截图，视觉/裁切只保留用户报告，不能声称有截图通过。最终窗口观察累计 9374 次采样（请求 50 ms，最大 gap 124.9 ms），目标退出并正常结束。

`UntilExit` 实际热键钩子请求 20 个有效样本，但在 900 秒上限到期时仅记录 **4 个非注入 Alt+Space 起点：2 个字符边界有效，2 个无字符边界/超时**；另有 1 个被标记并排除的注入热键。最终输入日志有 5 个 trusted、聚焦、可见、非 IME 合成字符，其中仅 2 个与有效热键边界精确匹配，另 3 个无法和物理起点配对。人工报告 150% 与恢复后的布局/输入正常，不能覆盖先前闪现/失焦失败，也不能替代未达数的 20 次分布。**热键稳定性、校准热键延迟、物理中文 IME 和帧呈现均未通过/未测。**

实际 DPI 与人工报告见[Wails DPI/输入核对](final-dpi-input.json)，96/144/96 窗口尺寸、显隐和前台逐条记录见[原生观察](final-wails-window-observations.json)。[观察尝试 JSON](final-input-observations.json)保留 5 起点、2 有效/2 无效和超时；[最终输入日志核对](final-session-report.json)说明 5 字符日志、2 条与热键时间匹配及正常退出。CSV/统计为[尝试明细](final-input-latency.csv)与[按分组汇总](final-input-latency-summary.json)。不将未配对字符行归入物理热键尝试。

窗口 observer 每 50 ms 轮询，捕获期间同时运行的热键 observer 以至多 512 条原生状态变化为界，并启用了非目标前台事件订阅；仅能确认实际采到的 DPI/窗口/前台转换，不能证明用户每次热键都进入了 hook。因此人工反馈、起点 observer 和 accepted-input 日志保留为互相独立的证据序列。

Computer Use 从已存在的隔离 exe 启动 PID 333632，启动调用返回“没有可操作窗口”，后续窗口清单亦未提供 Wails 窗口；测试探针只读状态却显示原生可见/激活、DOM 焦点就绪，扫描正常发布 619 项。这是桌面工具和进程状态两份不同观察，不把工具清单缺失解释为产品窗口没出现，也不凭 ready 判为实际输入。

用户按要求物理 Alt+Space 后确认“窗口出现，可以直接输入”。最终日志保留 4 条目标进程聚焦、可见、非合成、非 IME 合成、trusted 的字符接收，以及 1 条先前合成输入记录；不保存查询文字。本轮没有捕获物理热键起点，不能计算按键到实际输入时间，亦不构成完整物理 IME 或稳定性验收。此前闪现/不能输入的失败继续保留，不能被本次单次人工成功覆盖。`Chrome_WidgetWin_1` 焦点类名本身不足以断言是否存在宿主焦点故障，未据此定位产品根因。

用户确认后，桌面工具再次返回“实体 Esc 已停止”。此后 UI 输入为零，通过已有探针的鉴权 `/quit` 正常退出，持有进程句柄的会话确认退出码 0。该次未取得 Wails DPI 窗口事件，不填报样本。

2026-11-06 用户继续完成手动 Wails 对照，报告 100%→150%→恢复 100% 的布局和输入均正常。被动原生窗口采样独立验证第二屏右侧的实际 DPI/物理 bounds 变化（96 DPI 640×588、144 DPI 960×882、恢复 96 DPI）；系统缩放与监视器布局事前/事后一致。[逐条只读窗口观察](final-wails-window-observations.json)记录变化时间、窗口显隐/前台、物理 bounds 和最大轮询间隙。**此项仅覆盖原生窗口/显示缩放，Wails 窗口截图未取得，不能评审像素级布局或图标/字形渲染；工具物理 Esc 中断也使可见帧无法封存。** 用户报告保留为人工观察，不包装成图像证据。

此次右屏原生 144 DPI 时，窗口物理大小从 640×588 变为 960×882；恢复 96 DPI 后回到 640×588。用户报告该过程输入、布局均正常；这一部分作为人工观察，与缺少截图的像素级检查区分。

原生 144 DPI 区段窗口驻留在右屏 `(2341,168)`，从 96 DPI 的 640×588 重排为 960×882；系统缩放恢复后，原生 DPI 与 bounds 回到 96 DPI、640×588。人工报告另说明 150% 与恢复后的布局和输入正常。这仅证明该人工对照观察及原生 bounds 变化，缺少 Wails 截图，不按视觉像素检查通过处理。

原生 144 DPI 区段窗口驻留在右屏 `(2341,168)`，从 96 DPI 的 640×588 重排为 960×882；设置恢复后 native DPI 与物理边界回到 96 DPI、640×588。人工操作报告额外说明 150% 与恢复后布局/输入正常。系统显示配置原始与最终 monitor/DPI 行完全一致。[汇总文件](final-dpi-input.json)含人工报告和去关联限制，[原始 window observer 输出](final-wails-window-observations.json)及物理输入/输入日志逐条匹配见后续 CSV/JSON。

该进程最终退出码 0，exe SHA 未变。原生 50ms 窗口观察覆盖约 900 秒并正常终止于目标退出，最大实际轮询间隙 124.9ms。独立物理热键观察窗录到 4 个非注入 Alt+Space 起点（另有 1 个明确注入的工具快捷键，已排除）；其中 2 次匹配实际输入、2 次分别因重试/超时无效。5 个 accepted-input 字符日志都是 trusted、聚焦、非合成及窗口可见，query 不记录，但输入 hook 未和全部 Alt+Space 起点一一配对。`UntilExit` 请求 20 个有效边界后超时，禁止据此计算成功率；本次人工报告不撤销此前闪现/失焦失败，热键可靠性**仍未关闭**。校准后的 20 次热键延迟/可靠性、物理 IME 和结果帧呈现均未通过。

最后 CSV/摘要见[窗口/DPI 原始状态](final-wails-window-observations.json)、[观察尝试](final-input-observations.json)、[输入明细 CSV](final-input-latency.csv)、[分组统计](final-input-latency-summary.json)和[人工报告汇总](final-session-report.json)。4 个有效/失败起点和 5 行接受字符分开呈现，未尝试按时间邻近关系配对查询字符。

最终输入报告按 PID/创建时间核对完成：observer 请求 20 个有效字符边界，实际仅 4 个非注入 Alt+Space 起点（2 有效、2 无效）；另有 1 个注入起点排除。5 条字符日志中的 2 条各自与一个有效物理起点精确对应，其余 3 条不擅自配对。超时和物理 Esc/桌面采集边界都保留，因此用户本轮 150% 布局正常报告及字符日志不会被提升为 20 次热键稳定性通过。

最终 `UntilExit` 观察请求 20 条有效字符边界，900 秒时有 4 个非注入起点（2 条有效、2 条因重试/10 秒字符超时无效），另有 1 个明确注入并排除的起点；observer 本身未溢出，但有效数只有 2，分布要求未满足。日志共 5 条可信字符接收，仅两条与有效物理起点时间精确匹配，其他字符不能臆测为某轮热键样本。[最终观察尝试 JSON](final-input-observations.json)、[导出 CSV](final-input-latency.csv)和[分组摘要](final-input-latency-summary.json)保留 2 有效/2 无效，并标注未完成 20 样本门槛；人工报告、物理 hook 起点与字符日志作为不同观察保留。

最终原始 observer 文件重新导出 4 行：2 个有效、2 个无效，均不满足 20 次最低样本数；另有 1 个自动注入起点只作工具作用标记、排除。对 2 个有效计时终点按 `pid + unix_us + focused/native_focused/visible/composing/synthetic/trusted/utf8_bytes` 与落盘输入事件逐行匹配，均恰好一次；全日志 5 个实际字符事件均无丢失。CSV/汇总的缺样本和限制见[原始尝试表](final-input-latency.csv)、[统计](final-input-latency-summary.json)及[attempt/source 对应](final-input-observations.json)。这仍不能证明用户报告的每一步都被 observer 覆盖。

## 排除本轮三份资源样本

工具无法访问窗口时，曾调用可见资源入口取得 3 × 15 秒记录。旧入口自动填入“已扫描并预热首页/图标与中英文”的声明，而本次未实际完成这些预热；同时只按命令行分类，把通过环境/sidecar 启用的 `latencyprobe` 构建错误写成 `Production`。原始文件及其 `AllSamplesEligible=true` 保留，不回写旧哈希；**本轮三份样本在复核中整体排除，新增有效空闲样本为 0，原 54 份总数不变**。已测字节仍留作开发观察，不能混入官方无探针基线。

采集器已修正：

- `measure-rs12-session.ps1` 必须由调用方显式提供 `-IdleEvidence`，缺失或空白时在读会话前拒绝；脚本不再替操作者生成“已预热”声明，也不覆盖普通 Slint 模式的原观察来源。
- `measure-resources.ps1` 根据 Go 构建标签将 `latencyprobe` 标记为 `ProbeBuild`。这能证明程序包含探针，不能单独证明可选运行观察器是否启用；是否启用结合会话核对，不读出或公开进程环境中的其他数据。
- 回归覆盖缺少真实观察依据拒绝、官方 Go 构建保留普通分类、带探针构建不能称为普通模式，以及 Slint 测量/诊断模式分类。

验证、人工反馈、窗口原生事件、部分热键尝试、排除理由、原文件及更正工具指纹见[复核记录](review.json)、[验证](verification.json)与[清理](cleanup.json)。所有进程、测试 WebView、观察端点和临时热键已清理；两屏原 DPI/位置/工作区与正式 JSON 原文哈希未变。此补验仅改共享采集校验与说明，没有修改或重建冻结产品程序。

2026-11-06 final accounting: the native observer confirms the target window changed from 96 DPI / 640x588 to 144 DPI / 960x882 on the right display, then returned to 96 DPI / 640x588. The user reports layout and input were normal at 150% and after restore. No Wails screenshot or presented-frame measurement was captured.

The physical-input observer requested 20 valid character bounds but timed out with four non-injected Alt+Space starts: two matched a trusted focused visible character and two had no valid character bound (retry/timeout). One injected start was excluded. The accepted-input log has five lines; three are not paired with an observed physical start. No success rate is inferred, and earlier flash/focus reports remain unresolved. See final-input-observations.json, final-input-latency.csv, final-input-latency-summary.json, final-wails-window-observations.json, and final-session-report.json.
