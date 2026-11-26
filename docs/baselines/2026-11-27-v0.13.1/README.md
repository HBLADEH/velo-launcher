# Velo v0.13.1 发行验证

2026-11-27（北京时间），Windows 11 x64 26340.10032。使用 Rust 1.99.1、MSVC 14.50.35717 和 Windows SDK 10.0.26100.0，在干净候选提交 `4944fbefddd67e272eee7917a6b1163204cb6eee` 上运行 `./scripts/verify-rust.ps1 -Product -SmokeTest`。 [功能合入记录](https://github.com/HBLADEH/velo-launcher/pull/22)

- Software 和 FemtoVG 各通过 141 项 Rust 测试，按默认配置跳过 1 项 Everything 真实端点测试。两者均通过格式、Clippy、数据兼容、原生集成、PE 元数据、窗口、输入、业务、页面与重启冒烟。每份验证回执绑定 257 项源码输入。
- 合成用例覆盖桌面和嵌套开始菜单的 Steam 游戏与网页快捷方式，包括搜索、固定、增量索引和删除后刷新。桌面冒烟覆盖 `.URL` 导入、图标加载、重启后搜索和删除记录后保留原文件。
- 本机候选 exe 的 SHA-256 为 Software `6A9CC1753C9A1BB48AB02420DE03FCC3AFA421822A412E030209FAC99F4151BF`，FemtoVG `1E8BB51F0E59F22AFB8F1016178A089A8C954899767AB81CC56B61A1FD920923`。tag 工作流会重新测试、构建和打包，托管构建的身份及校验值由发行清单记录。
- 本轮使用合成快捷方式，实际 Steam 客户端启动游戏未复验。Windows 10、真实 IME、实体侧键、混合 DPI 和安装事务的结论以各自的构建记录为准。 [G4 机器验收](../2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md)

## 许可审查

- `audit-rust-licenses.ps1 -Product` 审核了 Software 322 项及 FemtoVG 324 项已解析依赖，回执对应上述候选 exe。Slint 继续采用 Royalty-free 2.0，设置 → 关于保留原生 Slint 致谢。 [许可选项与嵌套声明](../2026-11-07-slint-rs14/completion-2026-11-13/license-review.md)
- 相比 v0.13.0，锁文件只更新应用版本，第三方依赖、资源、许可证和锁定的 NSIS 版本一致。所有者于 2026-11-27 授权更新版本并以 tag 触发 GitHub Actions 发布。 [版本说明](../../releases/0.13.1.md)
