# Velo v0.11.2 正式发行

2026-11-20（北京时间），Windows 11 x64 26340，Rust 1.99.1、MSVC 14.50.35717、Windows SDK 10.0.26100.0。在干净提交 `ac11438a49474b118afe8c78f91b0b17a51c8465` 上运行 `./scripts/verify-rust.ps1 -Product -SmokeTest`，退出 0。[验证提交](https://github.com/HBLADEH/velo-launcher/commit/ac11438a49474b118afe8c78f91b0b17a51c8465)

- Software 和 FemtoVG 各通过 125 项 Rust 测试，Everything 真实端点专项各按默认配置跳过 1 项。格式、Clippy、数据兼容、原生集成、Release、PE 元数据、窗口、输入及页面冒烟均通过；各核对 229 项源码输入。
- 本机可执行文件 SHA-256：Software `78a603ba7476360e79ebcbc5d267c39b63f8586aea936e5ca1108555e39adf9d`；FemtoVG `a6bd51b38e189f86adce15bd3b0a2c32e94f5b4ba0d81123928c26b65b01d109`。这些身份对应桌面验收程序，保留原提交，不改标为后续托管构建。
- 两种渲染器的“自定义项目”和“关于”页面截图已检查，文件夹选择入口及 0.11.2 版本显示正常；导入、固定、设置、备份、删除及重启保留检查通过。页面检查使用隔离实例和合成数据。
- `audit-rust-licenses.ps1 -Product` 对 Software / FemtoVG 分别完成 322 / 324 项已解析依赖的许可审计。更新日志解析使用锁定的 `pulldown-cmark 0.13.5`、关闭默认 features，其 MIT 文本通过收集与校验。[既有许可审查](../2026-11-07-slint-rs14/completion-2026-11-13/license-review.md)
- 本轮不新增真实 Everything、物理 IME、多屏、混合 DPI、Windows 10、安装升级事务或性能验收。这些范围继续按各自历史构建记录理解。[历史机器验收](../2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md)

所有者于 2026-11-20 明确要求使用 GitHub Actions 公开发布 0.11.2。发行策略和版本说明随后同步；正式安装包由托管工作流在注释标签 `v0.11.2` 上重新构建，SHA-256 与源码提交随 Release 发布。`public-release` 环境已限定为 `v*.*.*` 类型的 tag。[发布工作流](../../../.github/workflows/release.yml) · [本版说明](../../releases/0.11.2.md)

## 正式发行结果

2026-11-20 04:05:45（北京时间），GitHub Actions 将 [Velo v0.11.2](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.11.2) 公开，`releases/latest` 返回该版本。注释 tag 绑定提交 `09e2a31310160ea4204c2dc7f7701531e6bea3e1`；发布正文与提交的版本说明一致。

- 发布提交的 [main CI](https://github.com/HBLADEH/velo-launcher/actions/runs/44599934148) 和 [tag 发布任务](https://github.com/HBLADEH/velo-launcher/actions/runs/44601558650) 均成功。两条托管流程分别对两个渲染器各通过 122 项测试，各过滤 3 项桌面测试、默认跳过 1 项 Everything 专项；本机桌面覆盖见上文。
- 正式安装包来自 `windows-2025` 托管构建，使用 Rust 1.99.1、Windows SDK 10.0.26100.0、NSIS 3.13，实际 MSVC 为 14.52.37025，编译器 / 链接器为 19.52.37029.0 / 14.52.37029.0。发行清单标记 `hosted-ci`，两种构建均绑定上述 tag 提交；不与本机桌面 exe 身份混用。[发行清单](https://github.com/HBLADEH/velo-launcher/releases/download/v0.11.2/release-manifest.json)
- 六项公开资产已实际下载，逐项通过校验清单及 GitHub API SHA-256 比对。默认安装器和兼容别名的 SHA-256 均为 `c781af7b1387424b65cafe6b19b028e3bc2065fce146b02d46636e047ab5487d`；FemtoVG 安装器为 `e764f4f3d10b173ef14e31d8f17527a0d0c383c6ae04abaf3af681de4ac905ae`。API 中第一个 `-installer.exe` 为兼容别名。[校验清单](https://github.com/HBLADEH/velo-launcher/releases/download/v0.11.2/SHA256SUMS.txt)
- 首次公开下载遇到一次 EOF，重试后六项文件均通过验证；未修改或重新上传发行资产。本轮没有执行正式安装器，不扩大安装事务和跨机器验收范围。
