# 启动组合：应用库配置调整

后续真实 UAC 的取消、重试同意及已提权进程识别已专项补验，见 [UAC 专项](../2026-11-16-launch-groups-uac/README.md)；本文件及 `verification.json` 保留此前构建与当时验收范围，不把新结果回填为旧轮检查。

日期：2026-11-16。分支：`codex/launch-groups`。基线 `63763d3`，当前为未提交增量，尚未发布。

用户澄清：组合只需在设置中展示应用库，并支持拖到成员区域；应用库应展示图标，添加按钮的 `+` 应居中。组合页不需要资源管理器文件拖放或文件导入。

## 实现

- 保留现有双区编辑器，成员只能从应用库拖入或点击 `+` 添加；移除组合页的文件添加按钮、外部拖入路由、批量解析及独立本地入口。现有“自定义应用”的文件导入不变。
- 应用库展示 24 px 应用图标及名称。可用应用全部显示，支持搜索和滚动，替换此前空查询只列出 80 项的截断。
- 复用已有后台图标工作者、generation 与 8 MiB 总预算。只加载可见来源行，滚动/搜索时更新可见请求，隐藏或退出编辑时释放模型内图片引用。
- `+` 使用现有 Fluent `add` SVG，按按钮尺寸水平/垂直居中；按钮继续支持点击和键盘操作，并提供“添加 + 应用名”的可访问标签。
- 组合仅存已有 `app_id`、名称与运行规则；执行从最新应用库快照取目标，不再回退独立保存的启动入口。特殊应用的“选择运行程序”只修改运行检测，不会新增成员，仍在后台校验所选 `.exe`。

## 本轮验证

Software 组件开发检查已通过：显示图标、内部指针拖动、重复去重、居中的 `+` 点击添加、三成员保存、放弃与取消确认。截图使用合成名称，图标来自 Windows 自带程序的只读提取；不执行这些程序，不作为目标第三方软件的兼容性证据。

真实应用专项另外使用本机 **7-Zip File Manager、7-Zip ZS File Manager、Google Chrome**。前两项启动前均未运行，分别对应不同目录中的 `7zFM.exe`；Chrome 已在当前会话运行，其快捷方式包含来源参数，测试明确指定 Chrome 主程序作为运行检测规则。首次派发 2 项、跳过 1 项，两款文件管理器的真实进程均出现；再次执行三项全部跳过。历史只有 2 条、各增加一次，组合查询未写入成员学习。结果见 `real-apps.json`。

真实软件检查通过本地临时 Rust 验证程序链接当前工程编译出的业务库，直接调用生产 `Runtime`、进程识别和 Shell 执行路径，使用独立数据目录；它不经过 UI 按键。库文件、验证程序及其源码哈希随结果保存。此前尝试呼出隔离预览时窗口失焦隐藏，未完成该次 UI 输入，因此不将其写成真实端到端 UI 验收。检查结束只停止本轮创建的两个文件管理器，原有 Chrome 继续保留。

两 renderer 的完整 `./scripts/verify-rust.ps1 -Product -SmokeTest` 均通过：各 **106 项 Rust 测试**、fmt、clippy、Go/Rust 兼容回归、原生检查、Release 构建及全部组件/业务/页面冒烟。随后两份编辑器检查、基于应用库的隔离原生批次检查也通过。移除了此前只服务于组合外部文件导入的 1 项测试，旧版 107 项结果不改标为新版结果。

最终回执与源码身份见 [verification.json](verification.json)：两份 ChecksPassed / SmokeTestPassed 均为 true，各 234 个输入指纹与当前源码一致。完整原始日志仅保留在忽略目录 `rust/target/groups-library-verification-b4487dec78cd40b7bbd13ad07f6f2e6b/`。

本轮截图为实际 Slint 组件、640×660、减少动画，源程序名称为合成数据：

- Software：[浅色编辑](screenshots/software-light-editor.png)、[深色编辑](screenshots/software-dark-editor.png)、[执行结果](screenshots/software-dark-results.png)。
- FemtoVG：[浅色编辑](screenshots/femtovg-light-editor.png)、[深色编辑](screenshots/femtovg-dark-editor.png)、[执行结果](screenshots/femtovg-dark-results.png)。
- 隔离原生结果：[Software](software-native.json)、[FemtoVG](femtovg-native.json)。

## 历史与范围

[此前基础实现证据](../2026-11-16-launch-groups-foundation/README.md)保留原构建和原范围。本轮删除了此前误加的外部文件配置能力，不将旧截图或 107 项测试的结果改标为当前构建。

前一轮专项测试在资源管理器多选前被用户按 Escape 中断，没有完成外部拖放或 UAC 测试；该轮未启动本机 7-Zip 或 Discord。本轮已完成上面的真实应用专项；根据本次澄清，外部拖放从当前组合功能范围中移除。UAC、Discord/更新器和受保护进程仍未验证。本机没有游戏或加速器，按用户说明记为不适用。旧专项预览已停止，旧测试数据仅位于 `rust/target/launch-groups-special-…/`。

本轮程序均显式使用独立实例和测试数据目录，未执行正式安装或发布。前一轮专项准备到本轮结束的只读哈希对照中，正式 `config.json` / `application-library.json` 相同，正式 `history.json` 有变化；其最后写入时间为本地 14:32:46，早于真实软件专项准备的 14:44:58，且正式 Velo 另有运行会话。本轮没有原文前后快照，不能具体归因或宣称正式数据全部字节未变，也没有覆盖或回滚它。后续证据继续区分组件/隔离 helper 结果与真实应用兼容性，不以截图或合成消息替代 IME、UAC、跨机器与性能验证。
