# RS-11 人工补验、修复与自动验收

日期：2026-11-04（北京时间）。接续 [2026-11-03 的 RS-11 验收](../2026-11-03-slint-rs11/README.md)。本轮使用独立实例和合成数据，不操作正式 Velo 数据或安装包。

后续发现的后台启动配置竞态及该快照的完整复验见 [后台启动续验](startup/README.md)；第一次真实登录失败、Run 命令长度保护和最新状态见 [登录会话续验](login/README.md)。下文两个修复的原构建身份和辅助样本保留原适用范围。

最终的 [真实 UAC 补验](uac/README.md)已通过两种渲染器的确认/取消、输入恢复及更新提权取消恢复，临时资源已清理；[G3 本机放行，RS-11 完成](g3-acceptance.md)。本页下文及原 JSON 保留当时检查点，不将历史构建改标为最终构建。

## 已发现的问题

1. Software 的真实“清空回收站”确认中，用户取消了永久删除，Shell 调用后旧代码却报告成功，导致错误学习一次历史。关机/重启取消未增加历史，取消后真实中文输入恢复。原始失败历史和用户确认已保存在本地，不能将原构建判为通过。
2. 安装器脚本使用规范化数据目录的 `\\?\` 路径启动时，Windows PowerShell 5.1 的 AuthorizationManager 直接失败，无安装或应用重启。将相同脚本入口换成常规 DOS 路径后，隔离安装样例确实以管理员令牌写入普通权限不可写的目录、校验 payload、重启原型并保留配置备份。

## 修复

- Velo 用所属原生确认框明确记录回收站确认/取消；取消不调用 Shell、不记录历史。确认后使用 `SHERB_NOCONFIRMATION` 避免第二个无法可靠判定取消的 Shell 确认，继续显示 Windows 清理进度并保留声音。空回收站只提示，未知查询结果仍要求确认；Shell 中断、取消和失败继续按真实结果处理。这是相对 Go 行为的明确修正。
- Windows 更新进程入口将本机 `\\?\X:\` 与 `\\?\UNC\` 脚本路径转为常规路径，保持路径字符和既有执行策略不变。`rs11_windows` 原生测试改为传入 `canonicalize()` 的路径，覆盖正常业务数据目录的实际形式。
- 新增 3 项回收站回归，确保取消/确认失败不调用清理、空回收站不清理、错误传播及并发清空后的重查；增加脚本路径转换测试。已有服务测试继续验证取消不学习历史。

## 本机限制和人工范围

本机 UAC 开启，但 `ConsentPromptBehaviorAdmin=0`，当前管理员可直接提权，不弹出同意/取消框。本轮不修改安全策略；提权成功和真实 UAC 取消分别记录，后者仍待会显示提示的环境。

用户确认 Software 的关机和重启确认可见并取消、回收站永久删除确认已取消、返回后中文输入正常。磁盘证据揭示回收站取消被学习，作为上述修复的实际触发样本。桌面工具不能捕获隐藏主窗口所属的确认框，弹窗可见性由用户报告，历史、进程、参数和令牌由本地数据独立核对。

前一轮用户不方便注销登录，当时未创建登录项。随后用户恢复协助，当前构建已通过设置页准备两个独立测试登录项，后续真实登录、热键及中文输入已验证且临时资源已清理，见 [登录会话验收](login/README.md)。隔离安装器仅为本地 fixture，不冒充已发布 Rust 安装包或 NSIS 发行验收。

修复后，用户再次取消 Software 的回收站确认，历史文件完整字节指纹未变；FemtoVG 的关机、重启、回收站取消均未生成系统动作历史，随后输入正常。FemtoVG 初次 helper 捕获为普通权限，单独重测第三项管理员菜单后得到管理员令牌，参数和工作目录均正确。上述范围与原构建分开列在 [assisted-results.json](assisted-results.json)，不把普通启动样本写成提权成功。

## 验证状态

修复后的 `./scripts/verify-rust.ps1 -SmokeTest` 完整入口退出 0。两种渲染器各 **79 项 Rust 测试、22 项固定组件断言、7 项业务组件断言、29 项完整页面与重启断言**通过。两种 Go/Rust 数据往返、NTFS 拒绝写入、原生 COM/Shell 对照及使用规范化路径的便携更新替换/失败恢复继续通过。原始日志为本地 `rust/target/rs11-assisted-20261104/verification.log`。

[build-identities.json](build-identities.json) 保留本节两个修复完成时两种构建各 140 个输入指纹；当时已与源码及 exe 逐项核对，两种输入指纹一致，渲染 feature 各自唯一。此后的后台启动改动使用续验目录中的新身份，不覆盖这份记录。

| 构建 | exe 原始字节 | SHA-256 |
| --- | ---: | --- |
| Software | 13,595,648 | `B0B7DBA4E7DA1F9FE5D6AC0370C180AAF883F9968E30849F0F92DB6205FC0DE8` |
| FemtoVG | 13,092,864 | `3189CD58174BF78AEFB116BCBF870037862A66EBC597386AC826ED7C1351C916` |

本轮继续在两份修复后的 Release 上运行真实旧数据副本的完整页面/重启流程。启动、查询、固定、导入、备份、设置保存、重启读回和删除通过，源配置、应用库、历史与索引文件的指纹不变。副本与原始日志只留在忽略的 `rust/target/rs11-auto-old-copy-<renderer>-20261104*`，不公开真实数据。

## 安装器自动验证

新增 [test-installer-update.ps1](../../../scripts/test-installer-update.ps1) 复用 `rs11_installer_fixture` 和生产的更新脚本生成/启动函数，每种渲染器各验证成功和失败两个场景：

| 检查 | 实际结果 |
| --- | --- |
| 提权与不可写目标 | 临时目录使用普通令牌只有读/执行权限的 ACL，真实写探针被拒绝；本地 installer helper 取得管理员令牌 |
| 成功替换 | 原目标使用另一种渲染器的有效 exe，确认安装后 SHA-256 改为请求的 payload，避免用相同字节证明替换 |
| 安装失败恢复 | 注入 helper 安装失败，原目标 SHA-256 不变；原程序携带隔离数据和后台参数重启，失败安装包、脚本及错误日志保留 |
| 配置与备份 | 四个场景的已保存配置和安装前备份原文 SHA-256 均保持不变 |
| 规范化脚本路径 | 调用生产 `updater::schedule`，输入真实 `canonicalize()` 路径，覆盖实际业务的数据目录形式 |
| 资源清理 | 仅移除匹配测试 PID 的托盘图标，再结束独立测试进程；在 `finally` 中恢复原 ACL 并逐项核对，没有遗留测试进程 |

结果分别见 [Software](software-installer-update.json) 与 [FemtoVG](femtovg-installer-update.json)，共四个场景均通过。这里的退出是测试清理，不冒充实际用户点击托盘退出；正常退出已有完整组件冒烟记录。

此入口只读取现有 UAC 策略；若环境会弹出提权确认，自动入口明确报未具备条件，不改安全策略或把未执行写成通过。手动例程与自动入口分别保留记录。复验示例（先在已初始化锁定工具链的 `rust/` 中构建 `rs11_installer_fixture` example）：

```powershell
./scripts/test-installer-update.ps1 `
  -Executable rust/target/artifacts/software/velo-slint.exe `
  -PreviousExecutable rust/target/artifacts/femtovg/velo-slint.exe `
  -FixtureHelper rust/target/x86_64-pc-windows-msvc/debug/examples/rs11_installer_fixture.exe `
  -OutputDirectory rust/target/rs11-installer-new
```

汇总见 [validation.json](validation.json)，公开证据的校验清单为 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)。2026-11-03 的原身份、样本和校验不变。本轮没有重采内存、CPU、GPU、启动延迟或完整视觉差异。

## G3 结论

当前长度保护构建各 80 项 Rust 测试、完整页面及旧数据副本复验通过，两种渲染器的真实登录自动启动、后台隐藏、热键与中文输入也已通过，详见 [登录验收](login/README.md)。用户手动启用 UAC 提示后，两种构建的真实确认/取消、输入恢复和生产更新脚本拒绝提权后的保留/重启全部通过；另一次真实确认的本地 installer fixture 完成替换与重启，所有测试实例正常退出并核对资源清理，见 [最终 UAC 证据](uac/README.md)。[G3 在本机范围放行](g3-acceptance.md)，RS-11 完成，可以进入 RS-12；正式 Rust NSIS 发行包按 RS-14 验收，默认渲染器未决。

所有原始日志、用户辅助检查点、失败样本和测试数据留在忽略目录 `rust/target/rs11-assisted-20261104/`。公开结果只保留合成标识和聚合字段。
