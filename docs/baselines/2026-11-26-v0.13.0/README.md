# Velo v0.13.0 发行验证

2026-11-26（北京时间）。Windows 11 x64 26340.10032，Rust 1.99.1、MSVC 14.50.35717、Windows SDK 10.0.26100.0。版本候选在干净提交 `7d0c9a32116261f9fbe83758aa9a51feb86a07e1` 上完成 `./scripts/verify-rust.ps1 -Product -SmokeTest`。 [工作台与启动组合合并记录](https://github.com/HBLADEH/velo-launcher/pull/21)

- Software 和 FemtoVG 各通过 140 项 Rust 测试，按默认配置跳过 1 项 Everything 真实端点测试。两份正式构建均通过格式、Clippy、数据兼容、Windows 原生集成、PE 检查、窗口、输入、业务、页面及重启冒烟，各核对 257 项源码输入。
- Software 候选 exe 的 SHA-256 为 `E3D85604FCE99A735CC7FC17EA56E129485072819BBDCC381CFB333786583ABC`，FemtoVG 为 `20A67887E4AF53D577051D6652D2F7930406D31B614912EF070E16F8A6FECB06`。正式安装包由 tag 工作流构建，其提交、渲染器与校验值见发行清单。
- 工作台默认关闭并作为测试功能提供。安装事务、真实 IME、实体侧键、混合 DPI 和 Windows 10 以各自的实机记录为准。 [工作台 v0.3 专项](../2026-11-25-workspace-v03/README.md) · [启动组合专项](../2026-11-25-launch-group-home/README.md) · [G4 机器验收](../2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md)

## 许可审查

- `audit-rust-licenses.ps1 -Product` 完成 Software 322 项、FemtoVG 324 项已解析依赖的审查，回执对应上列候选 exe。设置 → 关于保留真实 Slint 致谢，采用 Royalty-free 2.0。 [许可选项与嵌套声明](../2026-11-07-slint-rs14/completion-2026-11-13/license-review.md)
- v0.12.0 到本版的锁文件只更新应用版本，复用已有的 Windows API 绑定增补工作台所需特性。资源、许可文件与锁定的 NSIS 版本与 v0.12.0 一致。 [v0.12.0 许可记录](../2026-11-24-v0.12.0/README.md#许可审查)

所有者于 2026-11-26 授权推送小版本 tag 并触发 GitHub Actions 打包。 [版本说明](../../releases/0.13.0.md)
