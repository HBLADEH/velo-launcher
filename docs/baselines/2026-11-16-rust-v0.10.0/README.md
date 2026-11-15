# Velo v0.10.0 正式发行验证

2026-11-16。用户确认名称、托盘和底栏调整并授权以精确标签 **v0.10.0** 发布，引导旧 Go 用户迁移。新安装默认快捷键为 **Alt+Space**，迁移保留自定义快捷键。源码、文件身份与验收明细见 [发行验证清单](release-validation.json)。

两种产品构建来自干净提交 `a9f06efc14df6c525626183caa5a6ac9598d0e6d`，各 225 项源码输入、89 项 Rust 测试、格式/Clippy、Go/Rust 双向数据、原生功能、PE/CLI 资源、窗口/输入/业务/页面/重启冒烟通过。另运行旧 Go/Vue 完整回归，26 项前端测试和全部 Go 包测试通过。源码指纹同时核对工作文件和 Git 原始对象；发行包内文件、许可清单和 SHA-256 逐项核对。Software 为默认，FemtoVG 使用独立更新资产。

[本次 GitHub CI](https://github.com/HBLADEH/velo-launcher/actions/runs/43750461214) 对同一源码提交完成完整检查、两种正式包和产物上传，结论 success。公共安装器使用本机已实际验证的精确文件；CI 产物具有独立构建身份，不混用哈希。

隔离 Windows 10 客户机使用最终安装器完成并发拒绝、锁定/未知 Go 拒绝、官方 Go v0.9.11 原位接管、锁定 Rust 拒绝、替换后故障恢复、Software→FemtoVG 升级、Go 字节精确回退和卸载。额外验证已发布 Rust 0.1.0→v0.10.0 的升级，以及全新安装、正式名称/版本、没有 Go 快照时回退拒绝和卸载。结果见 [阶段汇总](batch.json)、[Rust 0.1.0 升级](rust-0.1.0-upgrade.json)、[全新安装](fresh-install.json)与[最终清理](final-cleanup.json)。合成用户数据原文未变，客户机没有 Velo 进程、活动注册或快捷方式残留，恢复档案保留；临时防睡眠请求释放，虚拟机恢复关闭状态，未修改电源设置。

提供 `velo-launcher-amd64-installer.exe` 兼容名称，字节与默认 `velo-rust-x64-installer.exe` 完全相同。旧 Go 更新器的检查、下载、安装脚本与发布标签 v0.9.11 一致；API 和限流订阅合成回归均验证发现 v0.10.0、选择 Software 兼容包及下载校验，Rust 回归验证保持对应 renderer。公开资产发布后再记录真实端点检查结果。免安装、管理员运行、可写目录及其他旧构建按[版本说明](../../releases/0.10.0.md)手动迁移，不提供可被旧更新器直接覆盖的裸 Rust exe。

两个安装器均含完整许可，Software/FemtoVG 清单与文本分别保存；缺失许可文本为 0，Slint 选择 Royalty-free 2.0 并保留 AboutSlint。公共资产包含两个系统安装器、一个同字节兼容名称、许可 ZIP、构建清单和校验清单，没有独立免安装版。

本轮安装验收通过已认证的 VMware Tools 后台接口运行于隔离客户机，使用提升后的安装进程，不代表真实 UAC 确认交互。宿主机当前 exe 通过自动组件冒烟；此前用户查看的 UI 开发截图保持[原构建身份](../2026-11-16-velo-0.10.0-space-keycap/README.md)。本轮未新增真实 IME、托盘鼠标点击、多屏/混合 DPI 或性能测量结论；历史 [G4/搜狗与机器范围](../2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md)继续保留自身适用身份。宿主机正式安装与数据未用于测试。
