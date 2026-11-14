# RS-14 当前机器接管与搜狗续验

2026-11-14 开始、11-15 完成（北京时间）。**RS-14 已完成，G4 在用户指定的当前机器范围放行，见[验收结论](g4-acceptance.md)。** 范围限定为现有 Windows 11 宿主机和该机中的 Windows 10 测试虚拟机；缺少的其他实体显卡/驱动、不可选 DPI 档位按用户要求跳过，不扩大兼容承诺。[完成状态](remaining-work.json)与[范围](scope.json)为本轮入口；前轮实际 CI 和独立数据副本回退见[原证据](../host-completion-2026-11-13/README.md)，保留原构建身份。

## 已实现和已核对

新增 `product` feature，与 renderer feature 独立；每个 exe 仍恰好包含一个 renderer。正式构建在无隔离参数的普通生命周期中，必须读取系统安装回执并匹配 HKLM64 的实际安装位置，才选择当前用户 `%LOCALAPPDATA%\Velo`。预览、组件冒烟和显式测试副本继续隔离；仅用命令行测试参数不能打开正式目录。原 Wails 单实例互斥量被占用时拒绝打开同一数据层。

系统安装器沿用官方 Go v0.9.11 的程序位置、`velo-launcher.exe`、HKLM 卸载项及全用户快捷方式。安装前复制并校验程序、注册值类型和快捷方式，写入同卷持久化事务记录；失败恢复或下次安装恢复未完成事务。保留原 Go 程序快照供 `uninstall.exe /ROLLBACK` 主动回退；Rust 卸载保留根目录未知文件和恢复档案。初次接管仅接受已核对 SHA256 的官方 Go v0.9.11 系统安装，不把未知程序当作可覆盖基线。

每个用户首次打开正式数据前，由后台数据线程为该安装事务建立原文快照，覆盖既有备份和未知文件；flush、重新读取并校验后发布。备份失败、损坏或目录链接会阻止打开数据层。系统安装器不以管理员身份遍历或改写各用户数据。再次安装产生新事务；旧快照保留。

快照限制为总文件数据 1 GiB、50,000 个文件和 32 层目录；超限会停止数据层初始化并提示检查，不跳过文件或删除原件。当前测试使用合成数据，不代表这些上界均做了满容量实测。

[正式构建身份](product-builds.json)绑定当前两份 exe、各 221 项输入与完整检查回执；源码/exe/metadata 哈希重新核对一致。每种构建的 85 项 Rust 测试、原生/数据契约、PE 资源、窗口/输入/业务/页面组件检查均通过。实际系统接管和搜狗物理输入另由下列独立证据验证。历史中断、断言错误和首次接管失败见[原尝试](prior-attempts.json)。

搜狗安装包来自官网链接的 HTTPS 下载，版本 `16.10.0.5858`、签名有效，见[包身份](sogou-source.json)。用户完成安装后，分别用两份相同当前 exe 的隔离身份/合成数据验证真实组合输入、候选框定位、翻页、方向键/空格选词、Esc 取消、Enter 提交及中英切换，并从托盘退出；两次均直接报告正常。[人工输入证据](sogou-acceptance.json)绑定 [Software 候选截图](software-sogou-candidate.png)、[FemtoVG 候选截图](femtovg-sogou-candidate.png)、74/69 次原生按键、组合/提交事件、实际退出码 0 和未改写的配置/历史/应用库。无误启动、组合期间隐藏或事件丢失；初次候选截图时尚无 winit IME 事件，后续提交事件另行确认，不将缺失片段伪造为已观察。

## 当前执行边界

首轮接管已验证锁定程序、未知基线拒绝，正常安装当时返回 30；该失败保持原结论。随后修复 Windows capability ACE 名称不能反向解析 SID 的问题，改为直接读取 ACL 中的 SID；本轮进一步修复继承 PowerShell 7 的 `PSModulePath` 导致 Windows PowerShell 5.1 无法载入系统模块，以及后者把 `File.Replace` 的空备份名绑定为空字符串的问题。分别以隔离[模块对照](ps-module-regression.json)、Windows PowerShell 5.1 下的[原函数测试](installer-functions.json)及实际系统安装复验。

[系统验收](machine-acceptance.json)包含四阶段 8 项安装断言：锁定 Go 拒绝、未知版本拒绝、Go→Software 原位接管、锁定 Rust 升级/卸载拒绝、替换后故障精确恢复、Software→FemtoVG 系统升级、原位 Go 回退、Rust 卸载保留未知文件/回退档案。三份普通权限运行会话均记录实际就绪、退出码 0、配置/历史/应用库原文未改。两 Rust 会话各有 22 文件的接管前快照；[日志指纹](machine-log-evidence.json)绑定普通权限、非空目录发布、备份和退出清理事件。

客户机系统安装通过已授权的 VMware Tools 服务会话执行，管理员令牌仅属于测试客户机；应用使用普通交互会话。最终轮没有用户 UAC 点击，也没有创建提权计划任务。保留原 UAC 黑屏/重启与工具失败记录，不能把服务会话替代路径算作新 UAC 人工通过。

最终 Velo 系统注册、全用户快捷方式、测试应用进程及提权计划任务检查为空；用户合成数据和回退档案保留。回退后的 Go 和两种 Rust 的退出来自原生托盘菜单自动调用，属于合成交互清理，不计物理输入。前轮 4 组真实 UAC 验收仍只适用于其原构建。

本地 workflow 已补上正式身份全量验证及系统候选打包，保留预览验证和隔离每用户安装；实际系统接管只允许测试客户机执行。该 workflow 修改尚未推送或运行，新构建不冒称已经过前轮 GitHub CI。

[验证汇总](verification-summary.json)补充旧版 Go 1.25.6 / Vue 回归、前端 26 项测试、lint/类型检查/构建，以及两 renderer 的 Go 1.25.6 双向数据差分。首次误用终端默认 Go 1.23 的 vet 失败原日志保留，切换固定版本后通过，没有修改测试绕过。预览两种完整检查保留其原回执，正式候选以本页当前源绑定为准。

[先前宿主机状态](final-host-state.json)保留自动项完成时的只读参考、分支和空暂存区。搜狗人工补验后的[清理](sogou-final-cleanup.json)确认无 Velo 进程/端点、安装注册、快捷方式或提权计划任务，原合成数据未变；[热键探针](sogou-hotkey-cleanup.json)确认 Alt+Space / Ctrl+Alt+K 可注册并立即释放。随后禁用运行期共享目录，通过 VMware Tools [正常关闭](sogou-guest-shutdown.json)测试客户机；原日志、数据和回退档案保留。

[证据审计](evidence-audit.json)与[公开校验和](SHA256SUMS.public.txt)提供文档链接、JSON、当前源码绑定和无凭据输出的检查结果。

本轮不发布 Release、不替换宿主机正式程序、不扩大实体硬件或性能承诺。内部构建 `0.1.0` 仍是重构候选版本，许可记录 `RedistributionCleared=false`；发布授权与技术验收分别记录。

已打包产物及绑定源码里的 `G4Passed=false` 是验收前的原值，保留原件和哈希；本次后续、限定当前机器的结论记录在 [current-machine-acceptance.json](current-machine-acceptance.json)，不倒改旧包或将新输入验收移植到旧 exe。
