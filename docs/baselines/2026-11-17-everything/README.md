# Everything 文件搜索实施与验证

本轮开发始于 2026-11-16，续至 2026-11-17（北京时间）。功能位于 Rust + Slint 主线，默认关闭、尚未发布；没有替换本机正式安装包。实现和操作边界见[功能说明](../../everything-file-search-proposal.md)。

## 实现范围

- 设置 → 搜索中的开关、可选程序路径及文件选择器，沿用草稿与原子提交。旧配置读取不重写，关闭后撤销查询。
- 官方 Unicode Query2 本机 IPC、惰性后台线程、一个在途查询与一个最新待查请求、两秒等待边界、过期回复校验、空闲回收。
- 独立 FileHit / FileEntry 模型、Tab 往返、中文及 Everything 语法、名称/目录/类型列表、50 项分页和 200 项上限、打开/定位/复制完整路径。
- IME、设置页和所属弹窗的输入优先；切换或隐藏释放结果，恢复查询与选择。文件结果不写应用索引或历史，不提取 Shell 图标或预览。

## 环境与身份

Windows 11 x64 当前宿主机，Software 与 FemtoVG 分别显式构建。工具链继续固定 Rust/Cargo 1.99.1、Slint 1.18.1、MSVC 14.50.35717 / 编译器 19.50.35719.0 / SDK 10.0.26100.0。

真实查询使用 Everything 1.4.1.1032 x64 官方便携版，ZIP SHA-256 为 698DF475EC44E638F66F1B6A32D28FEA613CEC78D3B6310E6ABE53431EEB940C，与官网校验文件一致；exe 的 Authenticode 状态为 Valid，发布者 voidtools PTY LTD。测试配置与索引都位于忽略的 rust/target/everything-tests/，只索引 125 个合成中文文本文件和一个合成文件夹，不导入真实用户文件。Everything 本体不进入 Git 或发行包。

最终 exe、源码输入指纹及回归检查结论保存在本目录 verification.json；组件检查、截图和资源样本以对应 exe SHA-256 为身份，不把较早构建改标为最终构建。

- Software：588D683E0B3CECABCFB2BD168234420B9CE0FEBEB4304CAECC35F4EC414982AE。
- FemtoVG：B81FDF8487A8914F1230446D1B01CC4703534E8565007F98A98A0C1B6E1ABBF4。
- [最终验证回执](verification.json)：每种渲染器各 241 个源码输入指纹与当前源码相符，ChecksPassed / SmokeTestPassed 均为 true。每种各 116 项 Rust 测试通过；各 1 项需合成端点的 live Query2 测试默认忽略，随后在准备好的合成索引上显式运行并通过。

## 验证内容

标准入口为 ./scripts/verify-rust.ps1 -Product -SmokeTest，包含两 renderer 的 fmt/clippy、Rust 单元/集成测试、Go/Rust 兼容往返、拒绝写入恢复、Windows 原生检查、Release、元数据、启动与全部现有组件/业务/设置页面冒烟。

新增的 scripts/test-everything-ui.ps1 在独立实例和数据目录运行真实 Everything 与 Slint：验证 Tab 往返、两种查询与选择恢复、IME/所属弹窗命令保护、50/50/25 三页、最新查询覆盖、隐藏结果释放、空闲 31 秒后活动查询线程为零、重新查询、设置页面暂停，以及开关关闭后持久化与资源清理。截图为实际 Slint 组件的合成数据，不是浏览器示意。

最终两份程序各 **18 项文件搜索组件断言通过**，测试与已有默认端点互斥，测试结束托盘和实例均清理：

- Software：[断言与 150 个独立资源样本](software-component.json)、[浅色文件列表](screenshots/software-files-light.png)、[深色文件列表](screenshots/software-files-dark.png)、[设置](screenshots/software-everything-settings.png)。
- FemtoVG：[断言与 207 个独立资源样本](femtovg-component.json)、[浅色文件列表](screenshots/femtovg-files-light.png)、[深色文件列表](screenshots/femtovg-files-dark.png)、[设置](screenshots/femtovg-everything-settings.png)。

独立 Query2 检查默认 ignored；准备合成索引后显式运行，确认中文多条件查询、总数、三页无重复、文件夹语法和零匹配。解析回归还覆盖截断、计数溢出、错误标志/排序/偏移、非法 UTF-16 和相对路径。应用索引、启动组合和旧配置的原有验证继续保留。

Windows 原生输入先行检查还观察到：真实 Tab 切换、中文文本查询、Ctrl+Enter 资源管理器选中目标文件、Shift+F10 菜单、复制路径与剪贴板一致、合成文件移动后保留查询并提示；真实拼音候选显示时 Tab 不切模式、Enter 不误打开文件。对应先行 Software exe SHA-256 为 6BB58C6049EE2A8292F3FCA72C2C3AD5DC4532CF07B1442E2A28C89C66ED8900；随后只收紧了文件错误提示文字。该原生观察保留自身构建身份，不作为最终两份 exe 的逐项物理输入证明。

## 资源口径与限制

资源 JSON 分开记录 Velo 与 Everything 的 PrivateBytes 和工作集，范围是包括启动、应用扫描、文件查询、截图、设置保存与 31 秒空闲的带探针组件检查。它不是待机基准，也没有拿不同构建相减来宣称内存收益。可验证的释放结论是结果模型清空、待查请求撤销、活动 IPC 线程归零，Everything 继续独立存在，测试结束只停止本轮创建的进程。

未覆盖其他 Everything 版本或命名实例、跨机器/多屏/混合 DPI、任意网络盘/权限/默认程序关联，以及所有 IME 的真实物理组合。软件不自动安装、修改索引或接管 Everything 服务；只有当前用户会话的默认 IPC 客户端才提供搜索。

## 失败样本与清理

- 初次只发现会话 0 的现有 Everything 进程，而用户会话没有默认端点，真实查询返回 NotRunning；这说明服务进程存在不足以认定“已连接”。
- 首次合成 INI 错把 folders 值写成独立一行，真实 Query2 成功但结果为零；修正配置后 125 项全部验证通过。
- 早期构建有未初始化资源编译器、接口接线及格式/clippy 错误，均在开发阶段修正，未计为通过。
- 一轮验证被预览进程占用 debug exe，另一轮原生预览占用与自检相同的快捷键；均清理自己的实例后重跑。模块拆分期间的一次中间构建缺少入口模块声明也保留在本地失败日志；最终验证使用冻结源码。
- 一次显式 Query2 检查晚于组件脚本退出，端点已清理而返回 NotRunning；重新运行独立合成端点后通过。

原始构建和桌面诊断日志仅保留在忽略目录 rust/target/everything-tests/；公开记录只保留构建身份、断言、合成截图和资源数值。测试创建的客户端、启动台和资源管理器窗口已清理，原有会话 0 的 Everything 进程及用户已有 Velo 实例不在清理范围。
