# Velo v0.12.0 发行验证

2026-11-24（北京时间）。Windows 11 x64 26340.10032，Rust 1.99.1、MSVC 14.50.35717、Windows SDK 10.0.26100.0。版本候选在干净提交 `8d6f45bf0b78a348e9984fb79f70b9d4ce2f166c` 上完成 `./scripts/verify-rust.ps1 -Product -SmokeTest`。 [验证提交](https://github.com/HBLADEH/velo-launcher/commit/8d6f45bf0b78a348e9984fb79f70b9d4ce2f166c) · [候选 CI](https://github.com/HBLADEH/velo-launcher/actions/runs/45344468943)

- Software 和 FemtoVG 各通过 139 项 Rust 测试，按默认配置跳过 1 项 Everything 真实端点测试。两份构建均通过格式、Clippy、数据兼容、原生集成、PE 检查、窗口、输入、业务、页面和重启冒烟，各核对 254 项源码输入。
- Software 候选 exe 的 SHA-256 为 `f3ec0b1febbe01310d26fe08067ee5793bcbc8623dd951346f35fa29c6dd778c`，FemtoVG 为 `cefcce28f6b0c755eb442440e6bfcc89da342ec77e3532ef60236cd9e6c68965`。正式安装包的源码提交及校验值随发行清单提供。
- 本轮使用隔离实例和合成数据，验证失效固定项的取消与移除、紧凑窗口的弹窗和焦点恢复、星标和空白处点击，以及重启后的数据保留。关机用例执行取消。回收站确认使用模拟回调。
- 径向工作台默认关闭，作为测试功能随本版提供。实体侧键、多屏、混合 DPI、Windows 10、真实 IME、系统安装事务及资源测量以各自的历史构建为准。 [工作台实机记录](../2026-11-21-workspace-v02/README.md) · [系统安装与输入法历史验收](../2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md)

## 许可审查

- `audit-rust-licenses.ps1 -Product` 分别对 Software 和 FemtoVG 的 322 和 324 项已解析依赖完成审查，回执绑定上列候选 exe。应用继续保留设置 → 关于中的 Slint 致谢，采用 Royalty-free 2.0。 [许可选项与嵌套声明](../2026-11-07-slint-rs14/completion-2026-11-13/license-review.md)
- 较 v0.11.2 新增的七个 Fluent 图标及其 MIT 声明已核对来源。包附第三方许可和依赖清单。 [上游资源](https://github.com/microsoft/fluentui-system-icons/tree/6c2c9075cf8952ef48369b2475233b6e9761efed/assets) · [MIT 许可](../../../ui/assets/fluent/LICENSE.txt)

所有者于 2026-11-24 授权通过 GitHub Actions 发布 v0.12.0，并将 v1.0.0 留待工作台修复后发布。发行流程使用锁定的 NSIS 3.13。 [版本说明](../../releases/0.12.0.md)

## 正式发行结果

2026-11-24 16:05:29（北京时间），GitHub Actions 公开 Velo v0.12.0，并将其设为最新正式版本。注释 tag 指向 `29d58206e2e8f5bc8322c7ac6dc62cb59588f960`，发布正文与该提交的版本说明一致。 [发布页](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.12.0)

- 发布提交的主分支 CI 通过。该提交在锁定 Windows 工具链上通过 `./scripts/verify-rust.ps1 -Product -SmokeTest`，Software 和 FemtoVG 各通过 139 项 Rust 测试及正式桌面冒烟。Everything 真实端点测试各按默认配置跳过 1 项。 [主分支 CI](https://github.com/HBLADEH/velo-launcher/actions/runs/45350773232)
- 本机使用锁定的 NSIS 3.13 从发布提交打包，发行清单将两种构建标记为 `desktop`。安装包预上传为草稿，tag 发布流程通过 `validate-tag` 和 `publish` 核验并公开草稿。 [发行清单](https://github.com/HBLADEH/velo-launcher/releases/download/v0.12.0/release-manifest.json) · [发布任务](https://github.com/HBLADEH/velo-launcher/actions/runs/45352994904)
- 六项公开附件已下载并逐项比对 GitHub API 的 SHA-256 和文件大小，`check-release-assets.ps1` 校验通过。`SHA256SUMS.txt` 的 SHA-256 为 `747f826bd83b1eec340a73f619f717c54a92d944906c70ebaddba52084b839f4`，与 tag 注释一致。默认 Software 安装器和兼容别名字节一致，兼容别名是 API 中第一项安装器。 [校验清单](https://github.com/HBLADEH/velo-launcher/releases/download/v0.12.0/SHA256SUMS.txt)
