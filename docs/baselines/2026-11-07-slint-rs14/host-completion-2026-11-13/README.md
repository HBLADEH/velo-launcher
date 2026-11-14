# RS-14 实际 CI 与 Windows 10 续验

2026-11-13（北京时间）。**GitHub CI、Windows 10 22H2 客户机的两 renderer 十组自动检查，以及官方 Go 系统安装→独立 Rust 数据副本→保留 Go 回退与清理均已完成。** CI 执行环境为本机的一次性 Windows runner；Software 仍为已测范围默认 renderer。自动同目录接管的正式 Rust 系统安装器没有实现，其他实体驱动/第三方 IME 等未测，G4 尚未放行。此前本机资源、真实 UAC 与回退见[上一轮](../completion-2026-11-13/README.md)。

## 真实 GitHub CI

用户授权将审查后的当前重构源码快照推送到临时分支 `codex/rs14-ci-validation-20261113-ea19bc0e` 并注册一次性 runner。最终[运行 43370007545](https://github.com/HBLADEH/velo-launcher/actions/runs/43370007545)对应提交 `4bab4c940fd01c7a1219cc4991ddeaadf5a37e26`，13:08–13:54 在本机交互桌面执行，GitHub 返回 **success**。当前工作分支和暂存区保持原状，未创建发布或替换正式程序。

两种 renderer 的完整 Rust/原生/组件检查、各五类 ZIP 拒绝、601/605 个文件校验、许可审计、固定 NSIS 准备、各八项隔离安装器和各六项实际每用户注册检查、产物上传全部通过。[运行及步骤](ci-run.json)、[产物身份](ci-artifact.json)、[exe/源码/回执](ci-build-identities.json)关联远端实际记录。下载的每个 ZIP 内文件再次逐项校验，构建各 214 项输入与当前源码一致，仅按 Git 的 CRLF→LF 归一化对照，没有忽略业务差异。

本次临时 workflow 用精确分支、`HBLADEH` actor 和唯一 runner label 限制本轮任务；保留 `contents: read`，不运行外部 PR，不发布 Release。其构建/验证入口复用仓库 `rust-preview.yml`；远端任务由临时分支 push 触发，不冒称默认分支的 `workflow_dispatch` 已执行。runner 未安装为服务，任务后自动删除本地凭据/注册，另行核对[清理](runner-cleanup.json)；临时源码分支与成功日志保留以便审查。

## CI 揭示的问题和修复

原工程 `rust-toolchain.toml` 只锁版本 `1.99.1`，本地验收靠目录 override 选择 MSVC host。首轮独立检出继承本机 GNU host，发现后取消并保留[原运行](prior-ci-attempts.json)。现已在工程锁定完整 `1.99.1-x86_64-pc-windows-msvc`，验证入口检查实际 rustc host；在无目录 override 的新目录也已核对。Rust/MSVC/SDK 版本没有升级，全局 GNU 默认没有改变。[修复范围](toolchain-fix.json)明确记录。

第二轮完整构建/ZIP已通过，NSIS 下载发生 HTTPS→HTTP 重定向，被 PowerShell 拒绝。没有允许降级或绕过哈希；准备脚本增加 `-ArchivePath` / `VELO_NSIS_ARCHIVE`，先按原 archive SHA256 校验，再核对 compiler SHA256/版本。正确包通过、错误包在输出创建前拒绝，见[离线校验](nsis-offline-validation.json)。第三轮使用此入口完整通过。

原失败/取消和额外工具告警保留：最终 Go cache 保存时 `tar.exe` 退出 `3221225477`，可选缓存未保存，发生于验证/产物上传后，不改变 GitHub 的 success 结论；Actions 的 Node 20 退役提示亦保留。没有将可选缓存成功写成已通过。

## 客户机直接控制与边界

本机已有 VMware Workstation 17。新建 Windows 10 22H2 简体中文 x64 测试机使用微软官方 ISO，整文件 SHA256 对照官网通过；新 growable 虚拟磁盘、4 GiB/2 vCPU，未挂载物理磁盘或既有虚拟机。客户机网络关闭，只共享只读测试 payload 与任务结果目录，宿主机正式应用数据未共享。

用户协助安装/修复 VMware Tools 并重启客户机后，`vmrun` 身份验证与进程查询通过，Tools 状态为 `running`。后续执行程序、复制文件、截图通过 VMware 客户机 API，不需要控制宿主机鼠标键盘或打开客户机网络。连接使用本轮随机生成的测试账户，凭据只留在忽略的任务目录，不进入公开记录。

首轮客户机脚本在 Windows PowerShell 5.1 读取子进程退出码时返回空值，保留为工具失败，未判为产品失败。改用本机同版本 PowerShell 7 的任务内副本继续执行；没有在宿主机安装或改动运行时。第二次等待卸载只观察 exe 消失，早于注册/快捷方式删除完成而拒绝；复核全部清理结果后，测试改为等待三个条件全部满足，新目录重跑通过。两份旧失败保持原结果。

## Windows 10 已验证范围

[环境](windows10-environment.json)为 Windows 10 Pro `10.0.19045`、VMware SVGA 3D `9.17.4.1`、1024×768/100% 的普通过滤令牌会话。[十组结果](windows10-results.json)覆盖两 renderer 的原生窗口、22 项输入组件、7 项业务组件、完整设置/导航/导入/备份/重启/删除页面，以及实际每用户安装、安装后窗口、锁定升级/卸载拒绝、升级替换和卸载保留未知文件/清理注册。程序来自最终成功 CI 的同一 exe，无产品源码或参数阈值变化。

[54 张组件快照](windows10-screenshots.json)及[逐日志 SHA/断言](windows10-log-evidence.json)保留对应关系；Software 业务窗口及 FemtoVG 深色设置已实际查看。数据/路径全部为合成测试资料。VMware Tools 的[直接控制检查](guest-api.json)、[PowerShell 身份](guest-powershell.json)、[媒体 SHA](windows10-media.json)和[审计](windows10-evidence-audit.json)可复核；[工具失败](guest-tool-failures.json)不以通过结果覆盖。

虚拟显卡环境不能代替其他实体 GPU、物理 IME 或性能预算验收；本轮不扩大性能承诺。

## 官方 Go 系统安装、Rust 副本与回退

[六阶段结果](official-go-rust-copy-rollback.json)已完成：用户确认客户机 UAC 后，微软签名的离线 WebView2 与原官方 Go v0.9.11 NSIS 安装包实际写入客户机 Program Files、HKLM 和全用户快捷方式；Go 实际前端展示合成库、用户托盘退出。保留这份系统程序/注册和原始配置备份，两种 Rust 每用户候选分别安装到测试目录、读取独立 Go 数据副本并正常退出，配置/历史/应用库原文字节不变，随后卸载 Rust。保留的 Go 再次实际显示，扫描日志记录 1027 项；用户从托盘退出，正常 `application stopped` 事件及无进程状态通过。

最后再次由用户确认客户机 UAC，官方 Go uninstaller 删除客户机系统程序、HKLM 注册及全用户快捷方式，合成用户数据与原始备份保留。[系统安装](guest-system-install.json)、[系统卸载](guest-system-uninstall.json)、[Rust 事件](migration-rust-events.json)、[用户反馈](guest-operator-reports.json)及[末尾清理](guest-final-cleanup.json)逐项绑定；宿主机正式安装、数据没有用于测试。

采集器的两次失败不覆盖：初次错误地按追加日志跳过行，而 Go 每次启动会轮换 `velo.log`；随后对已经附加观察的进程，在退出后没有取得可读退出码，旧脚本将空值与 0 比较而停止，故当时**没有发起卸载 UAC**。保留[失败原件身份](migration-tool-failures.json)，另按进程会话的有序启动/前端/关闭事件、用户直接退出反馈、无进程及数据/备份哈希验收。回退 ExitCode 明确为 `null`，不伪造 0；后续单独发起的实际卸载及清理通过。[生命周期诊断](rollback-lifecycle-diagnostic.json)、[审计](migration-evidence-audit.json)可复核。

这验证了**官方 Go 安装版与独立 Rust 内部候选之间可恢复的手动迁移路径**，不是自动同目录替换：当前 Rust 仍拒绝直接指向正式 `Velo` 数据目录，其 per-user preview 安装器保留独立 ID/注册。本轮没有绕过此保护或改名冒充正式系统安装器。G4 的剩余产品边界见[当前清单](remaining-work.json)，发布未执行。

虚拟机在结果归档后通过 VMware Tools 正常关机，运行期共享目录先禁用；[关机记录](guest-shutdown.json)保留。任务虚拟磁盘、原检查点与日志留下供复查，没有修改已有虚拟机。

[CI 证据审计](ci-evidence-audit.json)保留本轮适用范围；旧证据中的 runner=0/未推送是当时检查点，不再作为当前 CI 未完成的依据。

[最终证据审计](evidence-audit.json)、[宿主机清理](host-cleanup.json)、[G4状态](g4-readiness.json)及[公开校验和](SHA256SUMS.public.txt)提供本轮最终入口。宿主机五项正式文件仍匹配此前RS-14参考快照，当前工作分支与空暂存区保持；授权的临时CI分支留作审查，未发布Release。
