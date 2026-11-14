# RS-11 真实登录会话验收

2026-11-04（北京时间）。状态：**本机 Windows 11 x64 两种渲染器的真实登录、热键与中文输入均已验证，临时资源已清理**。第三次实际注销/登录后，两份程序均自动启动并保持后台；新登录标识、完整命令、进程创建时间、会话和配置原文字节核对通过。用户确认登录时未自行弹窗，两份均可用已保存热键呼出，无需点击搜索框即可通过中文输入法直接输入，Esc 清空并隐藏正常。结果见 [third-login-results.json](third-login-results.json)，清理见 [third-cleanup.json](third-cleanup.json)。

前两次用户也实际注销并重新登录，交互登录标识确已更换，但两份测试进程均未启动。失败分别见 [第一次](first-login-failed.json)与 [第二次](second-login-failed.json)，保留原结果，不改标为通过。

第一次使用 [后台显隐修复后的两种 Release](../startup/build-identities.json)，分别复制到中文/空格程序目录，独立 `--instance-id` 和合成数据目录均位于忽略的 `rust/target/rs11-login-assisted-20261104/`。不读取或替换正式 Velo 数据。Software 保存快捷键 Ctrl+Alt+J，FemtoVG 保存 Ctrl+Alt+U。

FemtoVG 通过 computer-use 操作常规设置页，Software 由用户辅助操作；均勾选“登录 Windows 时启动 Velo”并保存。原先不存在的两个测试 HKCU Run 值，其 exe、`--background`、实例和数据目录逐项匹配生产代码生成的命令，配置 `launch_at_startup=true`，可执行文件 SHA-256 与当前构建身份相符。原生只读观察确认每个 PID 只有一个 launcher 窗口且均隐藏。

Windows 的 [Run 文档](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)规定命令最长 260 字符；原两条命令分别为 **264 / 261 个 UTF-16 单元**，超出文档限制。注销前只核对了生成命令、数据隔离和注册表写入，未检查这一限制。两份配置原文字节未变，没有对应的应用崩溃事件或 StartupApproved 记录。没有在读取失败现场前手工重启程序，也没有实际热键/输入成功结论。

已用精确匹配的撤销脚本移除原先不存在的两个测试 Run 值，核对无对应进程；数据和失败样本保留。生产路径新增启用时的 UTF-16 长度校验，超长命令在注册表修改前失败，界面提示缩短程序或数据目录；保存已启用的登录设置时也刷新并重新校验命令，避免沿用旧无效值。关闭旧超长登录项仍可成功。回归覆盖边界、补充字符的 UTF-16 计数、拒绝后保留旧值，以及设置失败保留原配置、快照与热键。

这是相对旧 Go 仅以 Run 注册表写入成功判定启用成功的明确保护差异；不改变系统策略，也不创建额外常驻 helper 或计划任务来绕过命令长度限制。第一次注销前的 [prepared.json](prepared.json) 保留原样，适用范围仅是当时的配置准备检查。

第二次合成实例放到更短的忽略目录 `rust/target/rs11-login2/`，保留中文/空格路径与独立实例/数据。新的检查点将命令长度纳入注销前断言；新登录会话下先核对进程和隐藏状态，再请求用户逐一呼出、输入，并正常关闭临时登录项和测试程序。原始登录标识和本机完整路径只保存在本地检查点。

第二次的原本地视图检查保留在 [prepared-second.json](prepared-second.json)：命令各 194 个 UTF-16 单元，配置、热键与 exe 相符，两个窗口隐藏。但第二次真实登录仍未启动两份程序。进一步通过系统 `StdRegProv`、`HKEY_USERS` 和当前账户 SID 读取实际 Run，确认两个值都不存在，而当时本地 HKCU 视图包含两条完整命令。原检查漏掉了这层差异，不能据本地写入成功推断 Windows 会启动；长度超限是单独的保护缺陷，不能再把它作为第一次失败的唯一原因。第二次本地测试项已精确清理，实际系统项本来就不存在，数据及两次失败记录保留。

新构建的两种路径各 **80 项 Rust 测试、22 项固定/7 项业务/29 项页面与重启断言、三个原生后台启动场景**通过，数据双向兼容、原生发现及便携更新替换/恢复也通过。FemtoVG 在完整入口中完成全部步骤；同次命令后续的 Software 页面启动没有取得前台焦点，失焦隐藏后超时，该次命令退出 1。用户随后在前台终端执行 `verify-on-desktop.ps1`，其中 `./scripts/verify-rust.ps1 -Renderer Software -SmokeTest` 退出 0，再顺序完成 Software、FemtoVG 的真实旧数据副本页面/重启检查，原源文件指纹均未变。用户报告与落盘结果、全部日志及当前文件指纹已独立核对。

完整 [validation.json](validation.json) 和 [build-identities.json](build-identities.json) 保留当前两份 exe、各 141 个源码输入指纹及不同执行范围，后台场景见 [Software](software-background.json) / [FemtoVG](femtovg-background.json)。两种输入集合一致，与当前文件逐项匹配，渲染 feature 各自唯一。

| 构建 | exe 字节 | SHA-256 |
| --- | ---: | --- |
| Software | 13,598,208 | `B234714B46F5396B64FA08F2F6001CC4677C6FB03DBD01F52B579CBCD672EA3D` |
| FemtoVG | 13,095,424 | `4CE277451CE75027BCEAE8DC830519158FA6F047BC2040FB43AFCD9181F8A5D6` |

另一次误并行 UI 检查造成 Ctrl+Alt+K 争用，99 次预留失败和网格断言失败均保留原日志。注销后的 computer-use native pipe 连接失败，重置重试也未恢复，所以改用用户在前台启动脚本解决工具焦点条件；组件合成输入仍不算真实键盘或 IME 验收。安装器四场景保留 [上一份后台显隐快照](../startup/README.md)的身份，此次登录命令保护未重跑该可选 fixture，不改标为当前构建的安装器结果。第二次实际登录失败及后续系统视图核对继续保留，当前真实输入结论来自第三次的用户操作。

第三次继续使用上表同一份 Release，独立目录为 `rust/target/rs11-login3/`，两个新名字在本地及系统 Run 初始均不存在。用户从普通桌面终端运行准备的 launcher，然后在两份程序的实际设置页启用、保存并隐藏。程序创建的两条真实系统 Run 均为字符串类型、194 个 UTF-16 单元、精确匹配 exe/实例/数据目录；未观察到 StartupApproved 禁用标记。工具没有直接创建系统值来替代应用保存路径，详见 [prepared-third.json](prepared-third.json)。

此次桌面终端为管理员权限，CIM 返回了 PID 和创建时间，但当前普通令牌读到的映像路径/命令为空；不将其写成程序不存在。注销前用 desktop launcher 的 PID 记录和原生 `PROCESS_QUERY_LIMITED_INFORMATION` 查询实际映像路径，再核对所属 launcher 的隐藏状态，两个案例通过。第三次登录后的检查取得实际进程完整命令、创建时间和会话，并确认新登录标识、配置字节不变；没有使用注销前 receipt 替代自动启动证据，也没有在捕获前手工重启程序。

用户完成两种自动启动实例的真实热键与中文输入检查后，通过各自设置页取消登录启动、保存并点击“退出”。系统提供程序与本地视图均确认两个测试 Run 值不存在，配置保存为禁用；测试进程为零、原生实例端点消失，两组测试热键可重新注册并立即释放。没有注入按键，也没有强制结束这两份实例。合成数据与失败记录保留，未修改正式安装、UAC、StartupApproved 或安全策略。

本次输入结论限于用户当时的中文输入法及指定操作，不扩展为完整 IME、多机或性能验收。登录检查时 UAC 策略为 `ConsentPromptBehaviorAdmin=0`，当时 G3 尚待真实 UAC。随后用户手动启用提示，两种当前构建的 [真实 UAC 补验](../uac/README.md)已完成，最终 [G3 本机放行](../g3-acceptance.md)。本目录的原 JSON 保留登录完成当时状态，正式 NSIS 发行包仍按 RS-14 验收。
