# RS-07：文件拖放、导入预览与临时焦点

执行日期：2026-10-29，2026-10-30 完成协助补验（北京时间）。对应 [Rust + Slint 重构计划](../../rust-slint-refactor-plan.md)的 RS-07。

**状态：已验证（本机 Windows 11 x64）。2026-10-30 已补齐两种渲染器的单次多文件 Explorer 拖放、普通 Explorer → 管理员目标的权限边界、管理员同权限 OLE 拖放、管理员文件选择回退/返回输入及退出清理，见 [协助补验](followup-2026-10-30/README.md)。G2 继续未放行。** [RS-06](../2026-10-28-slint-rs06/README.md)暂缓的物理多屏、混合 DPI、拔插、休眠及 Shell 通知区实际点击继续保留，未开始 RS-08 业务或数据迁移。

## 构建身份与环境

沿用 `codex/rust-slint-refactor` 的已有 dirty 工作区，HEAD 为 `ac2f51ad1317f4abffab2c548af251e48ce16d2b`。保留既有 Go/Vue、共享采集工具和前序 Rust 修改，没有提交代码或替换产品安装包。Rust/Cargo 1.99.1、MSVC 14.50.35717、SDK 10.0.26100.0、Slint 1.18.1、静态 CRT 和两份独占 renderer features 保持原约定；只增加已有 `windows-sys 0.61.2` 的 Com / Controls_Dialogs features。

| 渲染器 | exe 字节数 | SHA-256 | 身份 |
| --- | ---: | --- | --- |
| Software | 10,864,128 | `2C26130F54BC6A38983A2F5C5BA7E75D2AFBE3199F4F65259731864FD6589D48` | [software-build.json](software-build.json) |
| FemtoVG | 10,362,880 | `2F12AFA94F84810F09D5F0D25F2CB54B69B2D5D1537964C94C2F996210404080` | [femtovg-build.json](femtovg-build.json) |

每份身份含 49 个源码、脚本与资源指纹，收尾已核对所有文件及实测 exe。初次实机检查发现 Slint 对未显式指定坐标的容器默认居中，导入标题与内容重叠；修复后重新完整验证，两份最终构建才用于本文证据。探索构建、原始日志与本地夹具位于忽略目录 `.reasonix/rs07/`；2026-10-29 的 20 张公开截图及 SHA 见 [screenshots.json](screenshots.json)，另有 [2026-10-30 的 7 张补验截图](followup-2026-10-30/screenshots.json)，未保留最初文件选择器展示用户文档目录的截图。

本机 Windows 11 x64，单屏 2560 × 1440 / 125%（120 DPI），工作区 `(0,0,2560,1380)`；系统信息、原生检查及清理见 [cleanup.json](cleanup.json)。2026-10-29 的两次最终运行都记录 `elevated=false`；2026-10-30 另完成两种渲染器的普通和管理员运行，权限由独立 token 查询核实，使用独立实例标识、数据目录和 `Ctrl+Alt+K`。没有改系统缩放、电源设置或权限过滤；没有重采内存或校准延迟，RS-04 的预算结论仍只属于原 RS-04 构建。

## 实现范围

- [main.rs](../../../rust/src/main.rs)将 winit OLE 的 `HoveredFile` / `HoveredFileCancelled` / `DroppedFile` 接入实际预览，不再只统计数量。隐藏窗口或原生交互期间的迟到拖放不能重新打开页面；诊断只记录来源、数量和状态，不记录路径。
- [import_preview.rs](../../../rust/src/import_preview.rs)提供会话内预览：支持绝对 `.exe` / `.lnk` 路径、中文和空格、粘贴时外层引号、大小写无关路径去重、缺失文件、目录及不支持的类型提示。文件检查延迟 40ms 合并同一批原生事件，进入容量 2 的后台队列，预览模型最多 256 项；仅在 UI 线程发布 Slint 模型。清空递增 generation，丢弃旧任务结果；后台结果可以更新草稿，但不能显示窗口或改变当前页面。
- 本阶段只检查类型、文件存在性及是否为普通文件，**不验证 PE 内容、不解析 `.lnk` 目标/参数、不启动文件、不读写应用库**。界面明确显示“预览仅检查文件”和“目标尚未解析”；后续共享数据协议、正式导入及 Shell 解析仍按 RS-08 至 RS-11 迁移。
- [app.slint](../../../rust/ui/app.slint)新增底栏入口与 640 × 660 导入页，含多选、路径输入、预览列表、原生菜单及返回。内容使用现有可滚动容器，路径与长名称省略显示；使用原主题与按钮。导入期间切到 Explorer 保持显示；Esc 返回原查询，主动关闭/热键隐藏后结束导入保持模式，预览与路径草稿仍保留在当前进程中。
- [interaction.rs](../../../rust/src/platform/windows/interaction.rs)在最多一个临时 STA 线程上打开有真实 owner 的 `GetOpenFileNameW`、`TrackPopupMenu`、`MessageBoxW`，结果回 UI 队列。文件选择设置 `OFN_ALLOWMULTISELECT`、`OFN_NODEREFERENCELINKS` 和 `OFN_NOCHANGEDIR`，保留选中的 `.lnk` 文件名；取消与系统错误分开处理。菜单可选择文件、清空预览或返回搜索，清空先打开确认框，默认取消。
- [lifecycle.rs](../../../rust/src/lifecycle.rs)按 owner 链、托盘菜单状态、显式导入模式和原生交互状态处理失焦。弹窗期间阻止应用显隐、设置切换及退出，避免父窗口先隐藏或销毁；弹窗返回后仅在本窗口仍属于实际前台时恢复控件焦点。普通主页失焦到无关窗口继续隐藏，未增加全局 IME 或进程级失焦豁免。
- 提权状态在导入页给出与普通 Explorer 拖入的边界提示，并保留“选择文件”入口；没有使用 `ChangeWindowMessageFilterEx` 等方式放宽 UIPI。

## 自动检查

[checks.json](checks.json)保留命令、日志指纹和关键结果。

| 检查 | 结果 |
| --- | --- |
| `./scripts/verify-rust.ps1 -SmokeTest` | fmt、两种 feature 的 clippy `-D warnings`、每种 26 项测试、两份 Release、4 次窗口/输入冒烟全部通过 |
| 组件输入冒烟 | 每种 20 项断言；新增导入接管焦点、Enter 不启动搜索条目、原生交互期间 Esc 不关闭导入、返回保留查询/焦点、设置结束导入保护 |
| 新增 Rust 行为检查 | 多选 UTF-16 路径、中文/空格及 `.lnk` 名称；无效选择结果；文件存在性/类型/目录；Windows 大小写去重；清空后拒绝迟到结果；导入/所属弹窗的失焦与按键边界 |
| `./scripts/verify.ps1 -SkipBuild` | 共享采集检查、Go vet/test、前端 lint/类型检查、26 项前端测试及构建全部通过 |
| 身份与事件 | 两份各 49 个指纹匹配；两次最终运行的测量事件全部 `dropped=0`，无路径或查询内容，见 [desktop-events.json](desktop-events.json) |
| 退出清理 | 两实例经底栏正常退出，均有 `desktop_stopped tray_removed=true`；无残留进程、主窗口或托盘端点；独立进程成功重获并释放 `Ctrl+Alt+K`，见 [cleanup.json](cleanup.json) |

组件合成事件和状态测试不替代原生输入。`.lnk` 测试中的存在文件可以尚未具有有效目标，因为当前契约明确为文件预览，不能将其标成完整快捷方式导入验证。

## 2026-10-29 本机实测

2026-10-29 的最终 FemtoVG 为 PID 10512，Software 为 PID 31748；补验使用独立运行，见下方补验入口。事件与原日志 SHA 见 [desktop-events.json](desktop-events.json)。界面输入通过 Computer Use 的实际键盘/指针操作完成，Explorer 拖放另由用户手动完成。

| 场景 | 结果与证据 |
| --- | --- |
| 两种导入页布局 | 均为 640 × 660 逻辑像素，内容与标题正确对齐：[FemtoVG](screenshots/femtovg-import-layout.jpg)、[Software](screenshots/software-import-empty.jpg) |
| 原生多文件选择 | 两种渲染器均实际选择中文/空格路径的 exe 与 lnk，返回 `paths=2`、无系统错误；原生前台归属期间 `owned_foreground=true native_dialog=true action=None`；[FemtoVG 预览](screenshots/femtovg-picker-multiple-preview.jpg)、[Software 预览](screenshots/software-multiple-and-invalid-preview.jpg) |
| 真实 Explorer 拖放 | 工具的跨窗口拖动被窗口范围限制拦截后，用户手动操作并回复“已拖入，出现 3 项预览”。记录到 **3 次分开的** `source=ole_drop count=1`，最终为 3 项 / 2 个支持文件 / 1 个 txt 无效项；[截图](screenshots/femtovg-user-explorer-drop.jpg)。这证明 FemtoVG 普通权限的真实拖入，未冒充单次多文件手势或 Software 实机拖放 |
| 导入失焦保持 | 切换 Explorer 后 `owned_foreground=false importing=true native_dialog=false action=None`，窗口保持显示，允许随后真实拖入 |
| 无效与重复文件 | txt 拖入显示不支持类型；Software 对带引号的缺失 exe 路径显示文件不存在；再次添加同路径后总数仍为 3；[缺失文件](screenshots/software-missing-file-preview.jpg) |
| 原生右键菜单 | 两种渲染器使用真实右键打开菜单，再真实点击清空预览项；菜单 owner 关系有效，未误隐藏；[FemtoVG 菜单](screenshots/femtovg-native-context-menu.jpg) |
| 系统确认及取消 | 两种渲染器都显示所属确认框，Esc 仅取消该框，仍保留 3 项预览；[FemtoVG 确认框](screenshots/femtovg-owned-confirmation.jpg)、[取消后](screenshots/femtovg-confirm-cancel-stable.jpg)、[Software 确认框](screenshots/software-owned-confirmation.jpg)。用户还在手动拖放前确认过一次清空，事件 generation 从 0 变 1，原文件保留后仍可拖入 |
| 弹窗期间热键 | FemtoVG 确认框前台时实际按 `Ctrl+Alt+K`，记录 `action=None native_dialog=true`，父窗口和确认框都保留 |
| 文件选择器取消 | FemtoVG 再次打开原生选择器并按 Esc，返回 `action=0 paths=0 error=false`，3 项预览保留；[稳定截图](screenshots/femtovg-picker-cancel-stable.jpg) |
| 返回搜索与输入 | 两种渲染器 Esc 返回后实际接受字符，同时满足组件/原生焦点和可见性；FemtoVG 保留原 `code` 并接受 `x`，Software 接受 `code`；[FemtoVG](screenshots/femtovg-typing-after-import.jpg)、[Software](screenshots/software-typing-after-import.jpg) |
| 导入保护结束及会话草稿 | FemtoVG 返回搜索后切到 Explorer，记录 `importing=false ... action=Hide`；真实热键呼回保留查询；重新进入导入仍保留 3 项。导入页 Alt+F4 只隐藏，呼回为搜索页；[草稿保留](screenshots/femtovg-preview-retained-after-hide.jpg) |
| 正常退出 | 两份程序均通过真实底栏退出；后续原生枚举和热键注册确认清理，无残留测试窗口或进程 |

截图来自窗口及其关联弹窗，通常以逻辑像素输出；实际 DPI/物理尺寸以原生日志为准。截图和工具调用耗时未用于宣称延迟、帧率或内存预算达标。

## Windows 回归矩阵与 G2

本表汇总第二阶段证据，既有实测不改标为 RS-07 新构建，也不将未实现的业务页、任务进度或配置持久化作为通过证据。

| 能力 | 已有通过证据 | 尚待验证 |
| --- | --- | --- |
| 热键 | RS-05 注册/冲突/回滚/录制/全屏；本项弹窗期间真实热键保护和最终释放 | 其他机器/全屏应用组合 |
| 启动与焦点 | RS-05 首次就绪、后台/冲突可访问；RS-06 真实热键和 Explorer 二次启动；本项弹窗返回实际输入 | RS-06 无前台权限的后台父进程请求激活边界仍保留原失败观察 |
| 显隐与会话 | RS-05 Esc 与关闭隐藏；RS-06 同 HWND/草稿；本项导入保护、结束保护和预览保留 | 完整业务设置草稿/任务状态按 RS-11 |
| 临时焦点 | RS-03 微软拼音；RS-05 所属窗口；RS-06 托盘菜单；本项真实右键菜单、选择器和确认框 | 第三方 IME 与其他 Windows 环境；完整业务上下文菜单按 RS-11 |
| 托盘 | RS-06 Explorer 实际重启恢复、原生菜单和退出清理 | 实际 Shell 通知区左右点击/键盘入口/高 DPI 图标直接视觉检查，按用户先前指示暂缓 |
| 单实例 | RS-06 双锁、独立目录、真实 Explorer 二次启动 | 正式产品数据切换按 RS-08 / RS-14 |
| 尺寸与位置 | RS-06 主页/设置/搜索动态高度、拖动/工作区；本项实际导入 640 × 660 | 物理小屏与完整业务页面 |
| 显示器与 DPI | RS-03 与 RS-06 的单屏真实 100/125/150/200% 和恢复；离屏注入明确标记 | 多屏/负坐标/混合 DPI/物理拔插/休眠，按用户先前指示暂缓 |
| 拖放与导入 | 两种普通权限 Explorer 单次多文件拖放、路径和原生多选；普通 Explorer→管理员目标正确阻止，管理员同权限 OLE 接收、选择文件回退/实际输入/清理均通过，见 [2026-10-30 补验](followup-2026-10-30/README.md) | 其他 Windows 环境；完整业务导入及快捷方式目标解析按后续编号 |

**G2 结论：不放行。** RS-07 的本机待验项已于 2026-10-30 补齐，状态更新为已验证；两种渲染器的普通/管理员权限边界、同权限实际 OLE、文件选择回退和清理已有通过证据。RS-06 暂缓的混合 DPI/多屏、拔插/休眠及 Shell 通知区实机项仍保留，原有后台激活失败观察也未改判。RS-08 仍以前置 G2 为准，默认渲染器留到 RS-14，不从本次交互结果推断性能选择。

## 2026-10-30 补验入口

[协助补验记录](followup-2026-10-30/README.md)保存权限矩阵、4 次 Velo 运行、2 次管理员 OLE 数据源、人工回复、7 张截图、完整事件及清理。普通 Explorer 与管理员数据源分别核实为 Medium / High；低权限拖入管理员窗口被系统阻止，同权限实际拖入与原生文件选择回退通过，没有修改 UIPI。产品源码与 exe 保持前轮身份，新测试夹具已编译并随附源码。

## 重现入口

```powershell
./scripts/verify-rust.ps1 -SmokeTest
./scripts/verify.ps1 -SkipBuild

# 正常生命周期；测试副本必须使用独立标识和数据目录。
./rust/target/artifacts/software/velo-slint.exe --instance-id=rs07-copy --data-dir=D:/velo-rs07-copy --hotkey=Ctrl+Alt+K --diagnostics
```

点击底栏“导入预览”，可粘贴路径、选择多个文件或从 Explorer 拖入。顶部空白处右键或“更多…”打开原生菜单；清空默认取消。普通权限与提权要分别记录；若普通 Explorer 无法拖入提权进程，检查界面权限提示和文件选择入口，不放宽系统权限过滤。测试结束先返回搜索，再用底栏“退出”，核对进程、托盘端点及测试热键释放。
