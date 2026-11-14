# RS-10：Windows 发现、索引、图标与启动

日期：2026-11-03（北京时间）。本轮将真实发现、后台刷新、可见图标与 Shell 启动接入独立 Rust 原型。正常模式使用 `%LOCALAPPDATA%\VeloSlintPrototype`，或显式指定的独立实例/数据目录；固定样例和原有测量入口继续隔离。正式 Velo 数据、Go 安装包和发布入口未切换。

验收限本机 Windows 11 专业版 Insider Preview `10.0.26340` x64。Rust/Cargo 1.99.1、MSVC 14.50.35717、SDK 10.0.26100.0、Slint 1.18.1 均沿用锁定值；Go 对照使用 1.25.6。截图为 100% DPI，合成查询和文件，不构成新一轮多屏/IME 或性能验收。G3 与默认渲染器仍未放行，下一项为 RS-11。

## 实现与调用边界

- `core/indexer.rs`：时间/大小指纹增量复用、快捷方式目标重检、深度规则、来源优先级、别名/ID/名称去重和辅助项过滤；不可访问子树保留旧条目，删除文件不再出现，瞬时解析失败下次重试。符号链接与目录联接不跟随，其他重解析点不会仅因属性位而被排除。
- `platform/windows/shell.rs`：Known Folders、同线程 STA、`IShellLinkW` 和原生 AppsFolder。打包应用枚举不运行 Go 或 PowerShell，不递归 WindowsApps。
- `core/service.rs`：发现、Shell 动作/历史写入、可见图标三个有界线程；刷新容量 1，启动仅一个未完成请求，图标邮箱只保留最新批次（最多 96 项），UI 通知按类型合并。过期/取消结果不发布；退出取消、唤醒并回收线程，原生同步调用在返回后观察取消。
- `core/icons.rs` 与 Windows 图像模块：兼容 Go 的签名和 `/icons/<hash>.png`，本地读取旧 PNG；只对可见项解码/提取，支持损坏缓存修复与文字降级。8 MiB 应用预算分为 4 MiB LRU 和 4 MiB 传输/解码/Shell 临时缓冲预留；可见引用受淘汰保护，隐藏/离屏模型释放图片。渲染器内部缓存另测。
- `core/launch.rs` 与 Windows 动作模块：普通启动保留 `.lnk` 的 Shell 语义，打包应用由 Explorer 打开；管理员目标保留 profile 参数和工作目录，打开位置取可执行目标的父目录。真实界面目前接通普通启动，其他动作的右键入口归 RS-11。
- `business.rs` / `ui.rs`：输入只查预归一化快照；启动请求一经接受立即清空查询，成功才保存历史；失败不恢复旧查询，生命周期 generation 防止迟到回调覆盖后来的显隐。设置与正式导入仍按 RS-11 接通。

有效配置/历史/应用库读取不重写；不兼容或损坏索引先保留原文副本，再重建。索引原子替换成功后才发布并清理图标，保留当前、前一代及自定义/固定所用图标；未知文件不清理。

两项明确差异遵循重构计划：Go 扫描会提前为全部条目提取图标，Rust 按可见范围加载；Go 的 `commitRefresh` 在索引保存失败时仍可发布内存结果，Rust 保留旧快照。前者有旧 PNG 兼容及可见模型验证，后者有真实 Windows 拒绝替换句柄测试。

直接使用已在 lockfile 中的 `windows 0.62.2`（Foundation/GDI/FileSystem/COM/Shell/Common）和 `image 0.25.10`（仅 PNG，关闭默认 features），许可证 MIT / Apache-2.0。`windows-sys` 的 Registry feature 是 ShellExecuteEx 结构依赖，本轮不修改登录启动项。没有新增常驻外部进程或服务。

## 验证与证据

最终汇总见 [validation.json](validation.json)，构建身份分别见 [FemtoVG](femtovg-build-identity.json) / [Software](software-build-identity.json)。工作区包含前阶段及本阶段未提交修改，源码身份以完整文件 SHA-256 为准；HEAD 不能单独代表这两份 exe。

公开文件校验见 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)。源码指纹标识原构建输入，公开文件清单校验整理/脱敏后的证据，二者不混用。

最终完整入口退出 0；两种渲染器各 **66 项 Rust 测试、22 项固定组件断言和 7 项业务组件断言**通过。各 116 个输入文件指纹完全相同，并逐项核对与当前源码及 exe 一致。

| 构建 | exe 原始字节 | SHA-256 |
| --- | ---: | --- |
| FemtoVG | 11,605,504 | `3973120A871FBE5E7F8891446CB6C704AC97F609BE7A3B2009FCEE8461893830` |
| Software | 12,107,264 | `C7AF72C1923152858E1C391EC710B938622E257F36CD1EF464FABB185414DC39` |

| 检查 | 覆盖 |
| --- | --- |
| `verify-rust.ps1 -SmokeTest` | fmt、两个 all-targets clippy / deny warnings、全部 Rust 测试、双向数据往返、原生对照、两份 Release 和三种冒烟 |
| 共享 Go/Rust fixture | 37 组名称；4 组来源/别名/profile/重复路径/过滤/动作能力，比较全部有序字段；原 RS-08/09 契约继续通过 |
| Rust 发现/服务回归 | 增量、修改/删除、桌面目标重检、深度、取消、独占目录保留、真实目录联接、PNG 复用/损坏/清理、LRU 引用、写失败不发布、自定义删除不复活、启动失败/UAC 取消不学习、历史写失败反馈 |
| 原生 Go/Rust 对照 | 合成 `.lnk` 的 COM 字段、文件时间/大小/ID、Known Folders、AppsFolder 与图标签名；本机 51 个打包应用一致，3 个样本图标成功读取 |
| 合成 helper 真实启动 | `.lnk` 参数和工作目录正确；旧 Go PNG 被复用；重复启动拒绝；成功历史落盘、文件移除后启动失败不学习；20 次刷新合并及重启读回 |
| 固定组件冒烟 | 每种原有 22 项断言；实际 HWND、非均匀 snapshot；FemtoVG 观察到 OpenGL 帧 |
| 业务组件冒烟 | 每种 7 项断言：真实目录查询/图标、组件导航、新查询保留、正常隐藏、迟到刷新不显示、会话查询保留、隐藏模型释放图片引用；正常退出并移除托盘 |
| `verify.ps1 -SkipBuild` | Go 1.25.6 下工具回归、绑定生成、lint、26 项前端测试、typecheck/build、go vet 和全部 Go 测试通过 |

本机 PATH 中 Go 1.23.0 的完整旧版回归在既有 `internal/update/public_test.go` 的 `%2F` 格式串 vet 检查失败；使用 RS-09 已采用且本机已有的 Go 1.25.6 重跑通过，未修改该测试或产品行为。原始日志只保存在 `rust/target/`，不提交。

最终源码的一次重跑中，`data_contract` 在输出 13/15 项通过后以 1 异常退出，没有失败断言或 panic 日志。同一二进制以 `--nocapture` 单独运行 15 项通过，随后连续 10 次（150 项）均通过，暂未定位原因；失败原件保留为本地 `rust/target/rs10-acceptance-verification.log`，未从验证历史中删除。最终完整入口结果单独保存在 `rust/target/rs10-acceptance-complete.log`，汇总中保留该异常及复验范围。

原生对照的公开汇总为 [FemtoVG](femtovg-native.json) / [Software](software-native.json)。真实应用列表只在合成临时目录参与离线比较；公开文件仅保留数量与通过项，不保存本机应用名称、个人路径或查询。

## 视觉检查和边界

以下截图来自最终 Release 的 `--business-smoke-test`，使用实际索引和可见范围图标加载；两种均为 640×210 的单结果窗口，已检查文字/路径省略、真实 PNG、选中态、深浅主题和底栏。底栏的部分目录警告保留，未将不完整目录访问伪装成全盘扫描成功。

公开图片仅遮盖副标题中的本机工作区路径，并明确标注“已脱敏”；原始 PNG 保留在忽略的 `rust/target/artifacts/<renderer>/business-*/fixture/data/`，布局与路径省略检查使用原件。

| 场景 | 截图 |
| --- | --- |
| FemtoVG 浅色 | [light](screenshots/femtovg-business-light.png) |
| FemtoVG 深色 | [dark](screenshots/femtovg-business-dark.png) |
| Software 浅色 | [light](screenshots/software-business-light.png) |
| Software 深色 | [dark](screenshots/software-business-dark.png) |

Computer Use 能读取实际窗口截图，但点击和重新选择后的显式激活均报 `failed to activate captured window`。因此业务按键证据来自明确标识的 Slint 合成组件事件，本轮不新增真实键盘/IME 验收。人工截图实例已结束；标准业务冒烟均经正常生命周期退出，测试实例、热键、托盘与业务线程已清理。

真实 UAC 桌面取消、管理员权限实际启动、打开位置的右键入口、正式导入/自定义管理、系统工具确认、完整设置和更新链路继续归 RS-11/G3。Windows 10、其他 IME/驱动边界沿用计划中的待验状态。本轮未重测内存、CPU、启动/热呼出延迟或长期增长，不将 RS-04 的旧资源数字标到新构建。
