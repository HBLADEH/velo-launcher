# 启动组合：真实 UAC 专项

2026-11-16，北京时间。分支 `codex/launch-groups`，基线 `63763d3` 上的未提交增量，尚未发布。Software / FemtoVG 两份产品 feature 业务库的真实 UAC 取消、仅重试失败成员、同意提权及已运行跳过检查通过，结果与身份见 [verification.json](verification.json)。

## 构建与方法

本轮没有修改产品源码。逐项复核[此前完整检查](../2026-11-16-launch-groups-library-only/verification.json)的 22 份改动源码哈希一致，沿用此前两 renderer 各 106 项测试、完整原生/兼容检查与冒烟结果，不因纯专项实测重复构建产品。测试程序分别链接现有 `product + renderer-software`、`product + renderer-femtovg` 的业务库；库、测试程序、helper、快捷方式和临时源码 SHA-256 记录在本目录身份文件中。

每份测试在全新隔离目录建立三个应用库成员。两个普通 helper 与一个带 Windows“以管理员身份运行”标志的 `.lnk` 均由本项目本地编译；helper 只记录自己的令牌、PID、中文参数和工作目录，不执行其他程序、网络请求、注册表或系统设置操作。helper 等待本目录释放标记后自行退出，并设置有限超时。

普通权限测试控制程序直接调用生产 `Runtime::launch_group`、快捷方式解析、进程识别和 `ShellExecuteExW`。真实 UAC 提示由用户在安全桌面点击，本轮没有自动点击、模拟 UAC 返回值或修改安全策略。系统策略前后均为 `EnableLUA=1 / ConsentPromptBehaviorAdmin=5 / PromptOnSecureDesktop=1`。

这一方式验证实际业务和 Shell 路径，不执行 Slint 界面按键，owner HWND 为 0；不把结果扩大为主窗口焦点恢复、设置草稿、真实 IME 或所有权窗口的端到端验收。

## 实测结果

两 renderer 均按同样顺序执行，详见 [Software](software-result.json)、[FemtoVG](femtovg-result.json)：

1. 先对组合执行一次，用户点击 UAC 的“否”。管理员项返回“已取消启动”，其 helper 未产生标记，历史也没有该项；另外两个普通成员收到请求并以非管理员令牌运行。
2. 只提交失败项重试，用户同意提权。此次只派发一个成员，确认其实际 token 为管理员；中文/空格/单引号参数与指定的工作目录完整保留。普通成员没有重新派发。
3. 在三个 helper 均运行时再次执行完整组合。普通权限控制程序能识别已提权运行的 helper，三项都返回 `AlreadyRunning`，零新增派发，无第二个 UAC 请求。
4. 最终历史恰好三项、各计一次，组合查询未污染成员查询学习；测试配置、应用库与组合文件原文字节不变。历史真实写入与 Shell 成功分别核对。

Software 与 FemtoVG 的取消、同意操作均已收到用户文字确认；实际管理员 token 与启动结果也由本地检查独立核对。FemtoVG 最后一轮“已点击是”的反馈在首次汇总之后到达，已补入身份文件的 `UserAcceptanceReported`，不改变原测试结果或构建身份。

## 保留的初始样本

Software 首次计划取消时，用户实际选择了“是”并主动说明。该次记录保留为[初始同意观察](initial-accept-observation.json)：三个成员均派发，管理员 helper 确认为管理员令牌，原本期待取消的开发断言因此失败。它没有计入取消通过；之后在新的 `software-retry` 目录独立完成取消与同意检查，没有覆盖原标记或日志。

准备重测时曾在触发请求前询问人工结果，用户报告“没有出现提示”；当时尚未写入触发标记，不算 Windows/UAC 失败样本。随后明确触发后重新收到用户“已点击否”的反馈。

## 清理与限制

[清理结果](cleanup.json)逐一核对三个控制程序和共九个 helper 进程均退出；包括初始选择同意的观察样本。通过 sibling release 标记完成退出，没有强杀管理员进程，UAC 策略保持原值。所有数据与原始进程标记只存于忽略目录，版本库结果不包含个人路径或原始启动参数。

本轮原始证据位于 `rust/target/groups-uac-assisted-e52b43af895f4069bba3ed36ed9fe0dc/`，包含本地临时 Rust 源码、二进制、分阶段标记、历史和原始日志。不替换正式 Velo，不安装/发布任何包。

结果限当前 Windows `10.0.26340` x64、当前用户及上述本地 helper。没有验证任意受保护进程、第三方带驱动或安装器的权限行为、多个成员同时弹 UAC、另一台机器或性能。两份业务库含各自 renderer feature，但此专项本身不渲染窗口，不以此推断两条 UI 路径的所有交互都已验收。
