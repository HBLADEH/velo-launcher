# RS-14 发行准备与 G4 缺口

2026-11-15 [G4 在当前机器范围放行](machine-completion-2026-11-14/g4-acceptance.md)：正式系统原位接管、失败恢复、升级、Go回退、卸载和清理，两 renderer 全量检查、固定 Go 1.25.6/Vue 回归及当前 exe 的两轮搜狗真实键盘通过。不可用实体环境按用户范围跳过，原预算差异/失败及构建身份保留；未发布。此入口取代下文旧状态，旧记录保持原日期和身份。

2026-11-12 已补齐本地 PE/许可/NSIS 与包级回退，见[最新续验](completion-2026-11-12/README.md)。下文与原 JSON 保留准备当时的构建/缺口，不代表当前自动验收状态。

用户下线后授权独立推进 RS-12/13/14。本轮完成可单独评审的构建验证回执、内部预览 ZIP、校验/拒绝测试和手动 Windows CI 配置。2026-11-11 [RS-12 本机测量矩阵已验证](../2026-11-04-slint-rs12/completion-2026-11-11/README.md)，原预算差异/历史风险保留；RS-13 与发行门槛仍未完成，**默认渲染器未决定，G4 不放行**。未替换正式程序、安装包、更新资产或登录项，未推送/发布。

构建、隔离启动、回退范围及 CI 环境见[发行准备说明](../../rust-slint-release-readiness.md)。本机检查与内部包验证结果归档在本目录；远端 workflow 没有触发，不将 YAML 就绪记成远端 CI 通过。

本机[完整检查](verification.json)通过：两种渲染器各 **80 项 Rust 测试**，以及格式、clippy、Go/Rust 双向数据、原生/窗口/输入/业务/完整页面组件检查；Go 1.25.6 的旧版 `verify.ps1 -SkipBuild` 通过。[Software 包测试](preview-software.json)与[FemtoVG 包测试](preview-femtovg.json)各拒绝五类错误输入，分别核对 ZIP 内 **544 / 548** 项文件校验和。两份清单均有 **29 项缺少包根许可文本**，不等于 29 项完全没有共享许可；完整审查仍未通过。包文件只在本地 `rust/target/rs14-preview-<renderer>-20261107/valid/` 保存，不加入 Git。各包身份与缺口见[Software](package-software.json)、[FemtoVG](package-femtovg.json)，总门槛见[G4 readiness](g4-readiness.json)。

## 当前渲染器依据

以下仍为 RS-12 原冻结业务构建的既有实测，不是本轮新 exe 的重测。三态各 3 个有效样本，完整范围和原始字节见[普通模式可见](../2026-11-04-slint-rs12/normal/resources-summary.json)与[普通模式隐藏](../2026-11-04-slint-rs12/remaining/hidden-resources.json)。

| 指标 | Software | FemtoVG | 判断 |
| --- | ---: | ---: | --- |
| 可见 PrivateBytes 中位数（MiB） | 26.32 | 55.03 | Software 达到 ≤50 预算，FemtoVG 超出 |
| 隐藏五分钟 PrivateBytes 中位数（MiB） | 26.12 | 67.30 | Software 达到 ≤30 目标，FemtoVG 超出 |
| 可见 CPU（整机 % 中位数） | 0.013000 | 0.058144 | 仅该采样窗观察 |
| 隐藏五分钟 CPU（整机 % 中位数） | 0.062086 | 0.006269 | 顺序/预热时点不同，不能声称 Software 全面更低 |
| 物理微软拼音 | 本机 96 DPI 四项通过 | 本机 96 DPI 四项通过 | 构建身份与范围见 RS-12 最新补验 |
| 严格冷启动/精确呼出/帧呈现 | 未齐 | 未齐 | 无法进行完整延迟/动画取舍 |

Software 是下一轮优先验证的内存候选。此建议不修改 Cargo 默认 features、不取消 FemtoVG，不把不同轮次模式的差值解释成探针开销或同轮收益百分比。Windows 10、不同 OpenGL 驱动和第三方 IME 仍不能从本机结果推断。

## 本轮工程交付

- `verify-rust.ps1` 开始时清除旧验证回执，成功后绑定 exe/身份/Cargo 元数据哈希；`SkipChecks` 与 smoke 范围分别记录。
- `package-rust-preview.ps1` 验证完整检查和 smoke、源码/元数据/exe 身份后，才创建全新输出目录；明确选择渲染器，只产出内部 ZIP。
- `test-rust-preview.ps1` 使用副本注入跳过检查、缺 smoke、exe/元数据哈希错误、源码哈希错误五种情形，必须在生成包目录前拒绝；再逐项核对有效 ZIP 内所有文件校验和。
- 包含独立预览入口、项目/资源许可、可找到的依赖根许可文本和缺口清单。依赖盘点是保守的 Cargo resolved 集合，含构建依赖；不等于所有项都链接到运行程序。
- Windows workflow 只允许手动触发，在匹配锁定工具链、可交互且空闲的自托管 runner 上串行检查两种渲染器、执行包测试和上传内部包；无发布步骤。

## 未放行项

NSIS 未安装，本轮未构建或验收正式 Rust 安装器。Windows exe 版本资源、产品版本/标签/资产同步、完整许可证文本及选择、最低系统/驱动要求、实际包级 Go→Rust→Go 升级/回退均待完成。既有数据双向兼容与更新脚本测试不升级为本轮 NSIS 证据。

RS-13 的同源码静态截图和动效差异已归档，但同轮原生 Wails 叠图、完整滚动与原生提示、动画录像等视觉签收仍待完成。RS-12 后续已补齐九份独立冷计时、暖启动/机器热键接收校准及结果/动画帧观察；这些观察不代替本页的视觉/发行门槛。历史 Wails 闪现/Software 首次失焦的原因仍未知，不宣称已修复或总体稳定性放行。原 `g4-readiness.json` 等保留本轮准备时的状态，不改写为后来构建的新验收证据。
