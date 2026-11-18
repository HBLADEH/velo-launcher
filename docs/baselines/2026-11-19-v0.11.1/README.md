# Velo v0.11.1 发行前验证

2026-11-19（北京时间），Windows 11 x64 26300，Rust 1.99.1、MSVC 14.50.35717、Windows SDK 10.0.26100.0。在干净提交 `07021736bf24fb9a7e143ae82a47bb9561416d2c` 上运行 `./scripts/verify-rust.ps1 -Product -SmokeTest`。[验证提交](https://github.com/HBLADEH/velo-launcher/commit/07021736bf24fb9a7e143ae82a47bb9561416d2c)

- Software 和 FemtoVG 各通过 121 项 Rust 测试，Everything 专项各按默认配置跳过 1 项。格式、Clippy、兼容性、原生集成、PE 元数据、窗口、输入和页面冒烟均通过。
- 两种本机 Release 可执行文件的 SHA-256 分别为 Software `08486a1b5d720c4093499c20f124d1f267041fb8c81bbe3ac89f3a517ce4f3b0` 和 FemtoVG `5386268f7e8b3968d4a7ef02b3a82ed6050aed0ea01f63c3b681fa2fb`。
- 运行 `audit-rust-licenses.ps1 -Product`，Software 和 FemtoVG 分别通过 322、324 项已解析依赖的许可审计。`Cargo.lock` 的版本差异仅涉及应用本身。[锁定依赖许可审查](../2026-11-07-slint-rs14/completion-2026-11-13/license-review.md)
- 当前安装目录选择的验证覆盖新装、旧 Go 登记、旧 Rust 默认目录和自定义目录。Windows 10 客户机、系统安装事务和实体输入法验收的覆盖以各自记录的构建为准。[安装目录验证](https://github.com/HBLADEH/velo-launcher/pull/10) · [历史机器验收](../2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md)

本记录中的哈希对应本机桌面验收可执行文件。发行安装包由托管工作流在版本 tag 上构建，其 SHA-256 与构建身份随 Release 发布。[发布工作流](../../../.github/workflows/release.yml)
