# Rust + Slint 发行准备

[开发说明](development.md) · [v0.11.0 版本说明](releases/0.11.0.md) · [验证入口](verification.md) · [文档索引](README.md)

版本来源是根目录 `Cargo.toml`。发行策略位于 `windows/release-policy.json`，记录指定版本的批准状态及对应证据。实际下载以 GitHub Releases 为准。

## 构建与验证

使用 `rust-toolchain.toml`、`windows-toolchain.json` 和 `Cargo.lock` 锁定的 Windows x64 工具链，在仓库根目录运行：

```powershell
./scripts/verify-rust.ps1 -Product -SmokeTest
```

此入口分别验证 Software 与 FemtoVG，每份构建只启用一种渲染器，并启用正式 `product` 身份。检查格式、clippy、Rust 测试、固化契约、原生集成、Release、PE 资源和窗口 / 输入 / 页面冒烟。`-SkipChecks` 只用于迭代，不能生成完整验收结论。

产物位于 `target/product-artifacts/<renderer>/`，其中 `build-identity.json` 绑定 exe、工具链、renderer features 与源码指纹；`verification.json` 记录成功的完整检查与冒烟。打包时重新核对两者及全部输入文件，修改源码后必须重新验证。开发身份和正式身份的回执不能混用。

真实 IME、多屏、DPI、Shell、UAC 与系统安装测试按各自证据范围执行。Everything 的真实端点测试使用独立环境，不接管正在使用的默认客户端；见[文件搜索说明](everything-file-search-proposal.md)。

## 生成本地系统安装候选

安装器使用 `windows/nsis-toolchain.json` 锁定的 NSIS。输出目录必须全新，已有候选保留原来的构建身份：

```powershell
./scripts/prepare-rust-packaging-tools.ps1 -OutputDirectory target/packaging-tools
$veloPackagingTools = Get-Content -Raw target/packaging-tools/tools.json | ConvertFrom-Json
./scripts/package-rust-product.ps1 -Renderer software -OutputDirectory target/candidate-software -Makensis $veloPackagingTools.Compiler
./scripts/package-rust-product.ps1 -Renderer femtovg -OutputDirectory target/candidate-femtovg -Makensis $veloPackagingTools.Compiler
```

候选生成不执行安装器。系统安装、升级、恢复、卸载和回退在隔离客户机验证；`prepare-rust-machine-install.ps1` 准备输入，`test-rust-machine-install.ps1` 与 `test-rust-machine-cleanup.ps1` 限定测试虚拟机。故障注入载荷仅用于恢复验证，不得分发。不要用宿主机正式安装和正式数据作为测试目标。

## 许可与数据保护

- 收集依赖及内嵌资源的许可文本，保留精确版本、来源与校验值；依赖清单包含构建依赖，不代表全部都是运行时组件。
- Slint 采用 Royalty-free 2.0，保留设置 → 关于中的真实 `AboutSlint`；项目 MIT 不替代第三方条款。
- 首次初始化前保留数据原文快照，备份失败停止初始化；更新继续要求 SHA-256 校验及安装前配置备份成功。
- 系统安装器保持安装回执、失败恢复和既有用户的升级路径；卸载保留用户数据及恢复档案。
- 资源测量保留构建、renderer、机器和采样范围。历史预算、历史结果与当前实测分别记录，不由一次打包扩大兼容性或性能结论。

## 正式发行资产

正式打包要求干净工作区和匹配版本的发行策略。打包前检查两种渲染器的验证回执均绑定当前 `HEAD`，同时重核输入路径和文件 SHA-256。本地打包可使用以下命令。

```powershell
./scripts/package-rust-release.ps1 -OutputDirectory target/release-0.11.0 -Makensis $veloPackagingTools.Compiler
./scripts/check-release-assets.ps1 -Directory target/release-0.11.0/assets
```

默认安装器为 `velo-rust-x64-installer.exe`，替代安装器为 `velo-rust-femtovg-x64-installer.exe`。兼容名称 `velo-launcher-amd64-installer.exe` 必须与默认安装器字节一致；发布时先上传兼容名称，再上传其余资产。发行清单、独立许可包 `velo-rust-<版本>-licenses.zip` 和三个安装器都必须列入 `SHA256SUMS.txt` 并通过校验。打包过程生成的载荷 ZIP 不是免安装版，不随正式资产分发。

产品 CI 在托管 `windows-2025` 上对 Software 与 FemtoVG 执行 fmt、Clippy 和 Rust 测试。tag 发布在同一镜像上增加 Release 构建、PE 元数据检查和 NSIS 打包。托管构建固定 Rust、Windows SDK、Cargo 依赖和 NSIS，并在发行清单中记录镜像实际使用的 MSVC 版本及 `hosted-ci` 验证范围。托盘、全局快捷键、原生确认焦点和窗口冒烟由独立的桌面验证覆盖。[产品 CI](../.github/workflows/ci.yml) · [内部预览验证](../.github/workflows/rust-preview.yml)

## 推送 tag 发版

1. **准备并批准版本。** 同步 `Cargo.toml`、`Cargo.lock` 和 `windows/release-policy.json`。审核对应版本的许可和实机证据，获得所有者的公开发布批准。将改动合入 `main` 并确认 CI 通过。可提交 `docs/releases/<版本>.md` 作为发布正文。
2. **推送版本 tag。** 在已批准且可从 `origin/main` 到达的提交上创建带注释的 `vMAJOR.MINOR.PATCH`。将 `$tag` 和 `$commit` 设为实际版本和提交 SHA 后，运行 `git tag -a $tag $commit -m "Velo $tag"` 和 `git push origin "refs/tags/$tag"`。
3. **查看 Actions 和 Release。** GitHub 托管 runner 下载并校验 NSIS，测试、构建和打包两个渲染器。若该版本没有手写说明，git-cliff 按 `.github/cliff.toml` 从上一个可达版本 tag 生成发布说明。发布 job 创建草稿，先上传 Go 兼容安装器，再上传其余资产，核对 GitHub 返回的 SHA-256 后公开。若 `public-release` 环境配置了必需审阅者，发布会等待审批。[发布工作流](../.github/workflows/release.yml)

Actions 中手动运行 `Publish Velo release` 可以预演打包。该入口只上传 Actions 产物，包含安装器、校验文件和未发布的更新日志。

如需发布本地验收过的同一份安装包，可用 `publish-rust-release.ps1 -Tag $tag -Commit $commit -Directory $assets -StageDraft` 上传草稿。命令使用有仓库发布权限的 `GH_TOKEN`，并输出 `Velo-Release-Draft: <SHA-256>`。创建注释 tag 时用第二个 `-m` 写入该行，托管发布任务会按 tag 里的校验值核对草稿资产和构建提交后公开。

检查未通过时，先查看 Actions 日志和 Release 状态。预上传草稿可以直接重跑发布任务，前提是仍然与 tag 绑定的校验文件和提交完全一致。已公开版本以新的版本号发布后续修复。

## 证据与历史

当前执行入口见[开发说明](development.md)，完整构建与实机证据见[基线索引](baselines/README.md)，既有 G4 范围见[机器验收](baselines/2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md)。新版证据保持自己的 exe 和源码身份，不改写旧结果。

0.1.0 首次发行期间的旧目录命令、迁移对照和阶段状态保存在[历史检查点](archive/rust-first-release.md)。
