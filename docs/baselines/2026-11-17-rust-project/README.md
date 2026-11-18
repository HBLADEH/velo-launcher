# Rust 主线整理与 0.11.0 发版准备

2026-11-17（北京时间），所有者确认 Everything 实测正常，要求 Go 版保存在 go 分支、主线成为标准 Rust + Slint 工程，并清理不再需要的构建与测试缓存。

## 已实现

- Cargo、src、ui、tests、examples、windows、licenses 提升到仓库根目录；既有 Everything 和启动组合修改全部保留。
- Go/Wails/Vue、旧 CI/构建及迁移期动态对照工具从主线移除；go 与 origin/go 均指向 ac2f51ad1317f4abffab2c548af251e48ce16d2b（最后 Go v0.9.11）。迁移期新增工具可从整理前 cc65619 检出。
- 固定数据/搜索/发现/页面 JSON 预期、Unicode/拼音静态数据和 MIT/BSD 许可保留；新增全量静态数据指纹校验。原生集成直接核对合成快捷方式字段、增量重用、缓存、Shell 参数、历史失败和重启。
- 预览和清理入口统一，开发数据移到 .dev-data，避免删除编译缓存时丢失测试设置。0.11.0 为本轮发版草稿，尚未发布。

## 已验证

`./scripts/verify-rust.ps1 -Product -SmokeTest` 最终退出 0。两 renderer 各 117 项 Rust 测试通过，另各 1 项要求隔离 Everything 125 文件索引的 opt-in 测试未运行；当前用户默认客户端在运行，本轮不改动该客户端。固定数据原文、全量静态表指纹、NTFS 拒绝写入、原生快捷方式/图标/Shell/历史/失败与重启、PE/manifest/依赖、窗口/输入/业务及完整页面冒烟通过。两份构建各 224 项源文件指纹与当前文件一致，清理后再次核对通过。

PowerShell 全部入口解析、现有测量工具回归、新增清理工具的预演/产物与数据保留/完整清理、校验文件损坏/兼容安装器不同字节/路径越界拒绝通过，当前文档链接和 git diff --check 通过。Go/Node/Wails 的实际构建调用已全部移除；预期 JSON、词典与第三方许可字节对齐整理前版本。

两份内部系统候选安装包生成成功，版本 0.11.0，依赖许可缺失为 0，未执行安装器。构建与 SHA-256 见 [verification.json](verification.json)。当前源码尚未提交，正式公开发行包必须在干净提交上重新绑定构建；没有将旧提交冒充本轮源身份。Software 为默认，FemtoVG 为替代。

本轮合成业务截图为 [Software 深色](software-dark.png) / [FemtoVG 深色](femtovg-dark.png)。原始 business-light 命名的首帧本轮也呈现系统深色，未将其记作浅色实测。历史 Everything 的真实 Query2、IME/Shell、深浅截图和所有者本轮手动验收保留各自范围，不能改标为本轮完整人工复验。没有重新验证 Windows10、新硬件、多屏性能或系统安装路径。

首次完整检查在版本元数据正则不接受 CRLF 时中断；修正为同时接受 LF/CRLF 后最终完整入口通过。数据拒绝写入检查中预期的 create temporary file 错误为失败回滚样本，不是最终构建失败。

## 清理范围

开始时 rust/target 为 150349 个文件、约 126.95 GiB；.reasonix 约 16.61 GiB。它们主要占磁盘空间，不是 Velo 常驻内存。清理旧中间产物、打包/测试副本、已关机测试虚拟机和本地采集缓存；公开证据和 .dev-data 保留。收尾 target 为 232,775,236 字节（约 221.99 MiB），.dev-data 约 1.17 MiB；以开始时两目录大小估算，旧生成文件净减少约 143.34 GiB。编译器缓存、旧测试虚拟机和 .reasonix/.playwright-mcp 已移除，frontend/build 空目录也已清理。保留两份当前产品构建、候选安装包、许可和锁定打包/测试工具。

两个旧 UAC 模拟安装目录的 107,665,055 字节（约 102.68 MiB）受保护文件现已清理。首次 Windows UAC 请求因未注意弹窗而取消；所有者要求再次触发后，本轮通过系统 PowerShell 的 UAC 确认，仅对两个明确的 target 测试目录重置 ACL，再由普通权限清理脚本移除。target 仅保留 previews、product-artifacts、tools；两份构建、224 项源码指纹及候选安装包的 SHA-256 收尾核对通过。没有修改正式安装、正式数据或用户 Everything 进程。
