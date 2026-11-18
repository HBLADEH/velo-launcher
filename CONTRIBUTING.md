# 贡献指南 · Contributing

[简体中文 README](README.md) · [English README](README.en.md)

## 提交问题与建议

欢迎通过 [Issues](https://github.com/HBLADEH/velo-launcher/issues) 报告缺陷或提出建议。项目以中文交流为主，也接受英文反馈。提交前请搜索已有问题，避免重复；新建 Issue 时可选择中文缺陷报告或功能建议表单，也可以直接新建空白 Issue。

缺陷报告请包含：

- Windows 版本、Velo 版本和安装方式（系统安装 / 旧版免安装）。
- 可复现的操作步骤，以及预期结果和实际结果。
- 相关截图或诊断输出；附上渲染器及安装 / 预览方式，便于确认运行环境。

请移除用户名、个人路径及其他敏感信息。功能建议请描述使用场景和希望解决的问题；较大的功能或架构调整建议先开 Issue 讨论范围。

## 开发与 Pull Request

1. Fork 仓库并创建主题分支，每个 PR 聚焦一个问题。
2. 按 [开发说明](docs/development.md) 配置 Windows 开发环境。
3. 修改代码并为行为变化补充适当测试；界面修改请附上实际截图。
4. 运行 `./scripts/verify-rust.ps1 -Product -SmokeTest`；界面开发预览可使用不带 `-Product` 的入口。安装器改动还需固定 NSIS 打包与隔离客户机验证。
5. 在 PR 中说明问题、修改后的行为、验证方式和已知限制，并关联相关 Issue。

仅修改文档时，请检查链接、命令和术语，无需执行完整程序构建。请勿提交本地日志、用户数据或 `target/` 中的构建产物。产品开发使用根目录的 Rust + Slint 工程，模块职责见 [Rust 工程说明](docs/rust-engineering.md)；历史资料入口见[文档索引](docs/README.md)。

## 文档与截图

- `README.md` 为主要中文入口，`README.en.md` 为英文版；修改功能、安装步骤或版本说明时请同步两者。
- 统一使用“启动器”“索引”“自定义应用”“固定”和“免安装版”等术语；引用界面按钮时保留实际标签。
- 使用真实界面截图，保存在 `docs/images/`，避免展示个人信息。截图的版本、场景和更新方式见 [截图说明](docs/images/README.md)。
- README 流程图由 `scripts/generate-readme-diagrams.py` 生成；修改后同步中英文，并运行 `python ./scripts/generate-readme-diagrams.py --check`。
- 区分已实现功能、设计目标和测量结果，不将未完成的验证写成性能承诺。
- 保留第三方资源的来源与许可信息。提交的贡献遵循项目的 [MIT 许可证](LICENSE)。

## English

Chinese is the project's primary working language; English reports and contributions are also welcome. Search existing [issues](https://github.com/HBLADEH/velo-launcher/issues) before opening a new one. You can use the Chinese issue forms or open a blank issue in English. Bug reports should include the Windows and Velo versions, renderer, installation/preview mode, reproduction steps, expected and actual behavior, and relevant screenshots or diagnostic output. Remove personal information before sharing. For substantial changes, discuss the scope in an issue first.

Fork the repository, create a focused branch, and follow the [development guide (Chinese)](docs/development.md). Product development uses the root Rust + Slint application. Add appropriate tests for behavior changes and screenshots for UI changes. Run `./scripts/verify-rust.ps1 -Product -SmokeTest`; installer changes also require pinned NSIS packaging and disposable-guest validation. Describe the problem, resulting behavior, validation, and limitations in your PR.

Documentation-only changes require checking links, commands, and terminology rather than a full application build. Keep both READMEs in sync. Regenerate README diagrams with `scripts/generate-readme-diagrams.py` and verify them with `--check`. Use actual UI screenshots in `docs/images/`, with no personal information. Do not commit local data, logs, or target/ build outputs. See the [documentation index](docs/README.md) for historical records. Preserve third-party notices; contributions are covered by the project's [MIT License](LICENSE).
