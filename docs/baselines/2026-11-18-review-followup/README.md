# 工程终审问题修复

2026-11-18（北京时间），在 Windows 11 x64 宿主机复验。保留现有单一 Rust + Slint 工程、双语 README 和演示截图；结果与[上一轮工程审视及截图](../2026-11-18-project-review/README.md)的构建身份独立。

## 回归范围

- `./scripts/test-project-tools.ps1`：模拟无关 Cargo 进程，确认它不阻断清理；检测到本 `target/` 内运行程序或锁文件被占用时拒绝操作。`-WhatIf` 不删除，默认保留当前产物，`-All` 保留 `.dev-data/`。另外在 Velo 的 Cargo 构建运行时调用实际 `clean.ps1 -WhatIf`，脚本识别了本目录的占用。
- 在合成发版环境调用正式入口：当前工作区脏、构建身份曾脏、第二种渲染器来自旧 `HEAD`，以及输入新增、修改或删除均失败，且未创建发行输出目录。完整输入由验证与打包共用同一枚举规则；实际 0.11.0 策略仍拒绝公开打包。
- 发行资产回归校验与清单版本一致的独立许可 ZIP。ZIP 损坏，或 ZIP 和相应校验项同时缺失，均被拒绝；兼容安装器别名和路径检查继续通过。
- `./scripts/test-everything-ui.ps1` 另行使用签名有效的 Everything 1.4.1.1032 x64 和 125 个合成中文文件。加载两页、选中第 80 项，分别验证隐藏再呼出、打开设置再返回时恢复 100 条及原选中路径；隐藏期间结果为空、空闲后查询线程已回收。基线仍区分合成组件事件和物理输入。

## 构建与验证

标准入口 `./scripts/verify-rust.ps1 -Product -SmokeTest` 退出 0。使用锁定的 Rust/Cargo 1.99.1、Slint 1.18.1、MSVC 14.50.35717 和 Windows SDK 10.0.26100.0。两种渲染器各通过 117 项 Rust 测试、格式、Clippy、固定数据契约、Windows 原生辅助检查、Release 构建、PE 元数据及桌面冒烟。各 1 项独立 Query2 测试仍按默认条件跳过。另行运行的真实 Everything 端点专项各通过 20 项组件断言。两种渲染器均人工检查了恢复后的截图：底栏显示 100 / 125，第 80 项选中且可见。

两份回执各核对 225 项输入 SHA-256，测试和 NSIS 3.13 内部候选包均与对应 exe 身份一致：

- Software exe：`1B796A174D16EFF918F02E589669D94FCB355E6BB2A69C729AD65BDDEF1BAC58`。候选安装器：`345FFC15A2AE5AC232FD44D98CEB83404891F807A25ACCFD847CE1F5F3E2E6EF`。
- FemtoVG exe：`32E3210350CCC1DA886718CD4C1CBCC7E1922FFDD3F7F16BEDAABED7E6815211`。候选安装器：`CF0E3740CD112FBEDB7017C77F60517B4183698259B56B9D7DE4834C23C3FF36`。

原始构建回执位于本机 `target/product-artifacts/<renderer>/`，专项回执与截图位于 `target/review-followup-everything-<renderer>/`。工作区基于 `cc656198b53ed049ba6abb13ff6f76738e247004`、含未提交改动，不是正式发行回执。仅专项测试进程已清理，原有 Velo 实例和 Everything 服务未变动。

本轮不复验 Windows 10、混合 DPI、物理 IME 或系统安装、卸载与回退，也不新增性能结论。未执行安装器，未批准或发布 0.11.0。
