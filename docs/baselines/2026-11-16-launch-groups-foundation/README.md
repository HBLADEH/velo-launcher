# 启动组合：实施与验证记录

此记录对应用户澄清配置范围之前的构建。最新组合配置仅使用带图标的应用库内部拖动，外部文件配置能力已移除，见[应用库配置调整](../2026-11-16-launch-groups-library-only/README.md)；以下历史身份与检查结果保持原范围。

日期：2026-11-16。分支：`codex/launch-groups`。基线提交：`63763d33f2468d792bcb947f4c49b08bec27e1d6`；本轮是在此提交上的未提交源码增量，不能把基线提交本身写成已包含该功能。

## 范围与状态

[策划案](../../launch-groups-proposal.md)已按用户要求改为“命名 → 拖入 → 保存”。本轮已接入组合数据、Slint 编辑器、应用库拖动、原生文件拖入/选择文件路由、搜索、按需进程识别、有界批次、结果、取消和重试。尚未打包或发布；真实游戏与外部拖放权限等边界按下面范围分别判断。

## 实现

- [组合数据与草稿](../../../rust/src/core/launch_groups.rs)区分应用库 ID 引用和组合独立的直接文件入口，取消编辑不会留下全局自定义应用。复用原子 Store，保留未知字段，损坏/未来版本原件不改写，未配置时不创建文件。
- [编辑器适配](../../../rust/src/group_editor.rs)与 [Slint 页面](../../../rust/ui/launch-groups.slint)提供连续拖入、点击添加、文件多选、去重、移除、草稿放弃确认、检测规则修复和结果查看。检查任务有草稿票据；关闭、切换或移除后拒绝旧结果。设置进入/返回保留原设置草稿。
- [进程识别](../../../rust/src/platform/windows/processes.rs)校验用户 SID 和会话，使用卷序列号与 128 位文件 ID 区分同名程序并识别硬链接。相关权限/身份失败返回未知，正常退出或已消失 PID 不误记为存活。使用受限查询权限、RAII 句柄和按需快照，不新增常驻轮询。
- [原生批次](../../../rust/src/platform/windows/group_launch.rs)最多三个临时 STA 工作者，保留完整结果快照，文件身份占位防重，已请求但未观察到的成员不会自动再次打开。单项失败继续；取消只影响未派发项；线程建立/执行失败最终也能释放忙碌状态。
- [服务](../../../rust/src/core/service.rs)复用启动/写入忙碌边界，管理组合与单应用的互斥，成功成员合并一次历史事务，组合查询不学习到各成员。真实启动与历史写入结果分开，不自动回滚或重试真实启动。
- 搜索只读取预生成内存索引，复用名称、拼音、别名与首字母匹配；与应用同名的精确结果保留应用在前，组合没有伪造的 Shell 执行路径。固定/常用应用数据格式不变。
- 只增加已有 `windows-sys 0.61.2` 的 ToolHelp/RemoteDesktop features，不新增 crate、不升级工具链或锁定版本；继续保留 MIT / Apache-2.0 许可。

## 已执行检查

宿主 Windows：`10.0.26340`，x64；没有将本宿主结果扩大到其他机器或文件系统。

1. 首个数据/进程增量：Software 库测试 48/48；两 renderer 的 `./scripts/verify-rust.ps1 -Product` 各 101 项测试、clippy、兼容/原生回归和 Release 构建通过。该增量尚无 UI，原 exe 后来被新构建覆盖；历史日志和下面的原哈希仅保留这一检查点的身份，不能充作最终产品结果。
2. 批次增量开发检查：Software 库测试 52/52，覆盖最多三个并发、部分失败、完整结果、取消、待观察防重，以及带参数宿主不能自动当作已运行。随后增加可选组合文件失败不阻断普通目录、保存/重启/搜索及真实进程部分结果的服务检查，计数以最终回执为准。
3. [编辑器组件检查](../../../rust/examples/launch_groups_editor.rs)：Software 的 Slint 指针拖动、重复去重、保存提交及放弃/取消确认通过，并检查浅色/深色编辑与结果截图。该指针派发是组件测试，不代表物理鼠标、真实 IME 或资源管理器跨进程拖放已验。
4. [原生启动检查](../../../rust/examples/launch_groups_native.rs)：仅执行本项目 [无窗口 helper](../../../rust/examples/launch_groups_helper.rs) 的隔离副本。预先运行一项；组合派发两项、跳过一项；执行中重复触发被拒绝；再次执行零重复派发，三个 marker 均只有一行；只有两项增加一次历史，查询学习为空；所有 helper 自行退出。
5. 真实鼠标检查使用两个独立预览构建和仅含本项目 helper 的自定义测试记录：命名后连续拖入三项，重复拖入仍为三项，保存后回到主页搜索命中；通过 Enter 发起三个 helper 请求，重新呼出可查看三项结果。随后退出测试预览，重启另一份 Software 预览，已保存组合仍可搜索。组合结果中多余的固定星标已移除，并在实际 Software 窗口确认；普通应用的固定能力保持原有行为。
6. 最终 `./scripts/verify-rust.ps1 -Product -SmokeTest` 的两 renderer 各 **107 项 Rust 测试**、fmt、clippy、Go/Rust 数据/搜索/发现契约、原生检查、Release 构建、固定样例/输入/业务/页面冒烟均通过。最后补齐组合页原生文件选择失败提示后，Software 在本轮入口中完成检查，FemtoVG 另以 `-Renderer FemtoVG -Product -SmokeTest` 完整复验通过；两份最终回执均为 ChecksPassed / SmokeTestPassed=true，各 **234 个源文件指纹与当前文件一致**，详见[最终身份与结果](verification.json)。

本地完整检查日志目录（忽略，不入 Git）：`rust/target/launch-groups-final-202c98b05562425489fdf773cba34881/`。原生检查输出：`rust/target/launch-groups-native-first/result.json`。

最终完整检查和两个 renderer 的补充检查位于 `rust/target/launch-groups-accepted-8afdb5f40d6340708c550495c1f0e484/`；最后一份 FemtoVG 完整复验位于 `rust/target/launch-groups-final-renderer-9e4d38f10c2c454ab120b3435bed994c/`。日志 SHA-256、最终 exe / 构建身份 / helper 哈希与改动源码指纹记录在 `verification.json`；不提交原始本地日志或 exe。

首个基础增量的旧 exe SHA-256：FemtoVG `B1C85AD691BCCF66B5F29E900B77152A2F61B1E121A0B90A60794B8C1CA5C831`，Software `0F7CC06D6F65AEDB76719A8E35DA162DB605258AF1744A951FE9189E88225B4E`。旧日志为 `rust/target/launch-groups-verification-e6401f70f9b048e2a6af8565a18f4c15/verification.log`，SHA-256 `12BAA4A7F1F848F2D9C42A7EFA138434B0ABC62EC81A5375BEA7E590D7A49F13`；此轮没有运行 `-SmokeTest`。

上述 GUI 交互构建的 SHA-256 分别为 FemtoVG 开发构建 `D83B4D5C1CCBC3C4E71F0C64AFF8327CAC5390EFBC758383109AD881E8B2F261`、Software Release `F290113C5CE9173F9C5542326FF4D2A6C6326ADA411663C892A0B12A5A50A4ED`；它们早于最后一次格式化，不改标为最终构建。两个预览均指定独立的 `group-review-20261116` 实例及 `rust/target/launch-groups-interactive-…/data`，保存结果为一组、三名成员，测试进程已退出。本轮未对已安装软件进行自动化启动。

## 组件截图与原生结果

截图来自实际 Slint 组件，以合成数据生成，逻辑尺寸 640×660、减少动画；不作为显示器帧时序、物理 IME 或跨权限拖放验收。

- Software：[浅色编辑器](screenshots/software-light-editor.png)、[深色编辑器](screenshots/software-dark-editor.png)、[深色执行结果](screenshots/software-dark-results.png)。
- FemtoVG：[浅色编辑器](screenshots/femtovg-light-editor.png)、[深色编辑器](screenshots/femtovg-dark-editor.png)、[深色执行结果](screenshots/femtovg-dark-results.png)。
- 原生隔离启动摘要：[Software](software-native.json)、[FemtoVG](femtovg-native.json)。两份均为派发 2 项、跳过 1 项，再次派发 0 项，helper 已全部退出。

补充检查的复验入口如下，每次使用全新输出目录；两 renderer 分别将 feature 设置为 `renderer-software,product` 与 `renderer-femtovg,product`：

```powershell
. ./scripts/lib/rust-environment.ps1
$veloGroupsEnvironment = Initialize-VeloRustEnvironment
Push-Location $veloGroupsEnvironment.RustRoot
try {
    cargo run --locked --no-default-features --features renderer-software,product --example launch_groups_editor -- target/groups-editor-fresh
    cargo build --locked --no-default-features --features renderer-software,product --example launch_groups_helper
    cargo run --locked --no-default-features --features renderer-software,product --example launch_groups_native -- target/x86_64-pc-windows-msvc/debug/examples/launch_groups_helper.exe target/groups-native-fresh
} finally { Pop-Location }
```

## 开发中发现并修复的问题

首次原生同名异路径测试遇到快照后退出的 PID，被归类为未知；增加退出确认，并使用 128 位文件 ID，保留相关访问失败的未知语义。

首次拖动组件检查失败：小列表在 Flickable 中默认居中，实际坐标与投放判断不一致；为内容指定原点。截图检查又发现未指定位置的页头/字段默认居中，已明确布局。随后拖动和三张截图均通过；这些失败是开发过程，不改标为最终通过。

最后新增的固定能力回调曾被格式检查拦下，运行 `cargo fmt --all` 后再执行完整入口；旧日志保留，不把未执行完的复验写成通过。

## 尚未验证的范围

未以本功能实际启动用户的游戏、语音或加速器；Steam/实际游戏主程序、反作弊、UAC 同意/取消等兼容性仍待目标软件验证。没有宣称应用就绪或加速完成。

资源管理器真实跨权限拖放、真实 IME、多屏/混合 DPI、另一台 Windows 10/11 和其他文件系统尚未做本轮复验。新增资源和延迟未测量，不声称达到性能预算。

未改正式用户数据、替换正式安装、打包或发布。测试副本与输出只在新建隔离目录，helper 自行退出，临时句柄由 RAII 回收；构建/原始日志留在忽略的 `rust/target/`。
