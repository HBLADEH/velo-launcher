# 文档索引

[项目首页](../README.md) · [English README](../README.en.md)

当前工程为根目录的 Rust + Slint 应用。日常开发以当前源码和下列开发说明为准。

## 使用 Velo

- [用户指南](user-guide.md)：安装、快捷键、搜索、自定义应用、备份、更新与数据目录。
- [系统快捷入口](system-shortcuts.md)：入口列表、操作语义与兼容范围。
- v0.13.0 启动器、工作台、宣传片分镜及构建与采集记录。 [界面截图](images/README.md)
- [实现流程图](diagrams/README.md)：中英文 SVG 及重新生成方式。
- [v0.13.1 版本说明](releases/0.13.1.md) · [v0.13.0 版本说明](releases/0.13.0.md) · [v0.12.0 版本说明](releases/0.12.0.md) · [v0.11.2 版本说明](releases/0.11.2.md) · [v0.11.0 版本说明](releases/0.11.0.md) · [v0.10.0 版本说明](releases/0.10.0.md)

## 开发与贡献

- [贡献指南](../CONTRIBUTING.md)：Issue、Pull Request、验证与文档约定。
- [开发说明](development.md)：工具链、构建、预览、测试、清理与打包。
- [Rust 工程说明](rust-engineering.md)：模块职责、线程边界、生命周期、隔离与兼容数据。
- [Everything 文件搜索](everything-file-search-proposal.md)：当前实现、可选依赖、失败处理与验证范围。
- [启动组合](launch-groups-proposal.md)：成员配置、运行检测、调度与重试语义。
- [Workspace v0.2](plans/velo-workspace-v0.2/implementation.md) · [径向窗口验收记录](baselines/2026-11-21-workspace-v02/README.md)

## 验证与发行

- [验证入口](verification.md)：自动检查和真实交互的适用范围。
- [发行准备](rust-slint-release-readiness.md)：系统安装、更新、回退及发行门槛。
- [跨机器与多屏复验](rust-slint-dual-monitor-testing.md)：历史 DPI、显示器及硬件兼容复核方法；当前构建命令使用开发说明。
- [资源测量](rust-slint-rs12-testing.md)与[界面动效验收](rust-slint-rs13-testing.md)：测量边界和历史验收方法。
- [基线索引](baselines/README.md)：构建身份、已完成验证、失败样本和未覆盖范围。
- [本轮工程审视](baselines/2026-11-18-project-review/README.md)：Rust 主线边界、文档整理、当前构建检查和截图身份。

## 历史资料的阅读范围

[重构实施方案](rust-slint-refactor-plan.md)保留阶段计划和按日期追加的验收决策。早期的 `rust/` 路径、迁移对照与旧状态描述属于当时构建，不是当前目录结构或当前操作命令。

[首次 Rust 发行检查点](archive/rust-first-release.md)保留 0.1.0 发行准备的日期、旧目录命令和阶段限制；当前发行操作统一使用上方的发行准备入口。

旧版源码、界面截图与构建说明位于独立的 [go 历史分支](https://github.com/HBLADEH/velo-launcher/tree/go)；迁移细节见[历史版本说明](legacy-go-wails.md)。`baselines/` 保留原始证据身份，`tests/fixtures/compat/` 保留数据与搜索的固化契约。发行兼容与第三方数据来源记录继续用于保障现有用户升级及许可完整性。
