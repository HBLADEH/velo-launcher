# RS-05：原生生命周期、热键与焦点恢复

执行日期：2026-10-28（北京时间）。对应 [Rust + Slint 重构方案](../../rust-slint-refactor-plan.md)的 RS-05。

**状态：已验证（本机 Windows 11 x64 / 微软拼音 / 系统 125%）。** 完成正式显隐、Alt+Space、后台启动、关闭隐藏、原生及输入焦点恢复、热键配置/冲突/重绑回滚、录制保护与全屏规则。Software 做完整交互走查，FemtoVG 独立完成后台呼出、实际字符/中文输入、Esc、关闭隐藏和失焦隐藏；共享原生逻辑在两条 feature 路径通过测试。下一步为 RS-06；G2 尚未完成，未开始产品业务或数据迁移。

## 构建与环境

分支 `codex/rust-slint-refactor`，HEAD `ac2f51ad1317f4abffab2c548af251e48ce16d2b`，沿用已有 dirty 工作区，没有提交、替换 Go/Wails 或写入产品数据。Rust/Cargo 1.99.1、MSVC 14.50.35717、SDK 10.0.26100.0、Slint 1.18.1 与独占 renderer features 延续前轮；新增直接依赖 `windows-sys = 0.61.2`（MIT OR Apache-2.0，原 lockfile 已有此版本）。

| 渲染器 | exe 字节数 | SHA-256 | 身份 |
| --- | ---: | --- | --- |
| Software | 10,620,416 | `5555606B628555E858CD40F3E7B27A170EF60936C24FBE2E10101A5622B7388C` | [software-build.json](software-build.json) |
| FemtoVG | 10,118,656 | `227C07B66C5FFA8C29DFB24BA5D8B021E34D03E19D04AFC6F23764858BB3DB44` | [femtovg-build.json](femtovg-build.json) |

两份身份各包含 42 个源码/脚本/资源指纹，覆盖新原生模块与桌面夹具；收尾逐文件核对当前输入与实测产物一致。实机矩阵只引用上表构建；修复前的探索构建及失败走查留在忽略的 `.reasonix/rs05/`，没有把旧日志改标成新 SHA。

系统为 Windows 11 专业版 `10.0.26300`、x64；保持原系统 125%，客户区 800 × 675 物理像素对应 640 × 540 逻辑像素。没有设置 SLINT_SCALE_FACTOR、修改系统热键或更改 DPI。默认渲染器仍未选择，RS-04 的内存结果只属于原 RS-04 构建，本项没有重采资源或宣称新增内存收益。

## 实现

- [lifecycle.rs](../../../rust/src/lifecycle.rs)统一处理首次就绪、可见/隐藏、录制、退出与转换代次。Slint 先创建隐藏 HWND；固定模型就绪、窗口注册完成后才决定首次显示。`--background` 仅在注册成功后保持隐藏，注册失败时显示可访问的窗口。
- 显隐保留同一个 Slint 组件与原生 HWND，保留查询、当前面板、主题、减少动画状态及热键草稿。隐藏暂停 presented 状态并移走 TextInput 焦点；事件循环使用 `run_event_loop_until_quit`，最后一个窗口隐藏不会结束驻留进程。关闭、空查询 Esc 和热键显隐进入同一转换。
- [window.rs](../../../rust/src/platform/windows/window.rs)在 Slint UI 线程调用原生激活及 SetFocus。后台父进程无前台权限时，仅在首次尝试失败后临时关联前台输入队列并立即解除；没有模拟 Alt 键或 Wails 的假宿主焦点跳转。首次激活另以 `GetForegroundWindow + GetFocus` 同步真实焦点，日志标为 `native_focus_sync`，与 winit 的 `native_focus` 事件分开。
- 延迟焦点回调核对转换代次、可见性、关闭状态及真实前台归属，不能重新显示或抢回已隐藏窗口。失焦后只安排一次 30 ms 的归属复查，避免在 Windows 尚未切换前台时误判；没有持续轮询。当前 HWND、子窗口或 root owner 属于启动器时不自动隐藏，其他前台在热键可用时隐藏。录制在真正失焦后结束。
- [hotkey.rs](../../../rust/src/platform/windows/hotkey.rs)使用独立原生消息线程，空闲阻塞于 GetMessageW；容量为 1 的配置请求队列，无每次按键新建线程。`MOD_NOREPEAT` 阻止按住重复触发；事件按消息顺序回到 UI 线程。重绑先获得新注册再释放旧注册；失败保留旧键，成功后旧键立即释放。消息 ID 与 lParam 的键码/修饰键均需匹配，过滤快速重绑后旧 ID 复用的过期事件。退出注销并 join 线程。
- 设置面板提供输入/录制/应用及“全屏时禁用呼出”。已注册组合键被 Windows 消耗时，转发给录制器并仅填草稿；普通录制经原生键盘事件取组合，Esc 取消。应用才更改注册；草稿及开关仅保留在当前进程，配置持久化归 RS-08。
- 全屏比较前台窗口与其所在显示器的完整边界，排除普通带标题栏的最大化窗口、桌面及任务栏；只阻止从隐藏状态呼出，已显示时仍可用热键隐藏。
- 注册失败时保留失焦/Esc 后的可访问性；此时关闭可以退出。正常注册时关闭只隐藏。RS-06 加托盘前，底栏“退出”是明确退出入口，不把正常关闭改成退出。

## 自动检查

完整结果与日志指纹见 [checks.json](checks.json)。

| 检查 | 结果与边界 |
| --- | --- |
| Rust fmt、两种 feature 的 clippy `-D warnings` | 通过，含桌面夹具 target |
| 两种 feature 的 cargo test | 各 16 项通过；包含就绪次序、后台恢复、迟到焦点、连续转换、录制、全屏几何及原 Go 热键解析契约 |
| 真实 Windows 热键测试 | 原生注册冲突、失败后旧绑定仍可派发、成功重绑释放旧键、12 个连续消息、旧 ID/旧 lParam 过滤及 Drop 后重新注册通过；派发消息为测试注入，单独保存实际键盘证据 |
| 两种 Release / 窗口与输入冒烟 | 通过；每条路径的窗口/非空 snapshot 与 15 项组件输入断言通过，FemtoVG 观察到 NativeOpenGL；不把合成输入当作 IME 实测 |
| `./scripts/verify.ps1 -SkipBuild` | 通过，包含共享采集、Go vet/test、Vue 类型检查、前端测试与构建 |
| 实际隔离输入采集 | Arm → Capture → 实际输入 → Alt+F4 → Snapshot 通过，`Valid=true`；保留 ComputerUseAssistedUpperBound 限制，没有因正式关闭隐藏而遗留进程 |
| 退出清理 | 正常退出后另一进程成功取得 Alt+Space；测试夹具随后退出，最终没有 velo-slint/rs05_desktop_fixture 进程，见 [cleanup.json](cleanup.json) |

冲突走查的 F23 占位夹具必须先退出再运行原生热键单元测试；收尾曾因该夹具仍持键产生 `os error 1409`，释放后重新检查通过。这是测试占位互斥，未修改测试跳过冲突，也未把该失败归入通过样本。

## 实际桌面矩阵

使用本机 computer-use 技能与 `@oai/sky` 的真实 Windows 输入。原生事件、PID、代次及原日志 SHA 保存在 [desktop-events.json](desktop-events.json)，截图指纹见 [screenshots.json](screenshots.json)。截图均为测试窗口和固定样例；没有用户配置或实际应用路径。

| 场景 | Software | FemtoVG | 证据 |
| --- | --- | --- | --- |
| 普通首次显示前保持隐藏，实际字符进入搜索 | 通过，原生焦点在工具输入前已由 Win32 确认 | 后台首次呼出覆盖，普通启动由窗口/输入冒烟覆盖 | [Software 首次输入](screenshots/software-first-input.png) |
| `--background` 注册成功不弹窗，Alt+Space 呼出并实际输入 | 通过 | 通过 | [Software](screenshots/software-background-input.png)、[FemtoVG](screenshots/femtovg-background-input.png)，日志均先 hidden/startup 后 shown/hotkey |
| 后台注册失败仍显示，Esc/失焦不失联，可关闭退出 | 通过：第二进程请求已占用 Ctrl+Alt+K | 共享逻辑测试通过，实机详验使用 Software | [失败回退](screenshots/software-background-conflict.png)、pid 24892 的事件 |
| Alt+F4 只隐藏，再呼出保留查询及原 HWND | 通过 | 通过，保留中文查询 | [Software](screenshots/software-close-recalled.png)、[FemtoVG](screenshots/femtovg-close-recalled.png) |
| Esc 有查询先清空、空查询再隐藏，下一次主页呼出实际输入 | 通过 | 通过 | [主页呼出输入](screenshots/software-recall-input.png)、两份生命周期日志 |
| 失焦到无关前台后隐藏 | 通过 | 通过 | `lifecycle_blur_check owned_foreground=false action=Hide` 及 hidden/blur |
| 所属原生弹窗前台不误隐藏 | 通过 Win32 owner 夹具 | 共用归属判断 | [父窗口](screenshots/software-owned-popup.png)、[关联弹窗](screenshots/software-owned-fixture.png)，日志 owned_foreground=true/action=None |
| 录制当前已注册键不隐藏；普通录制新键 | 通过 | 共享原生与状态测试覆盖 | [当前键录制](screenshots/software-record-current.png) |
| 显隐保留设置页、查询、减少动画及未应用快捷键 | 通过 | 共享组件/状态路径 | [草稿恢复](screenshots/software-draft-preserved.png) |
| 新键重绑成功；占用键应用失败，旧键仍可隐藏/呼出 | 通过 Ctrl+Alt+K；冲突夹具持有 Ctrl+Alt+Shift+F23 | 两种 feature 的原生测试通过 | [冲突](screenshots/software-rebind-conflict.png)、[原键再次呼出](screenshots/software-conflict-recalled.png) |
| 全屏禁用开启时不呼出，关闭该设置后允许呼出 | 通过真实覆盖显示器的无标题栏窗口 | 共用原生判断及状态测试 | [阻止](screenshots/software-fullscreen-blocked.png)、[允许](screenshots/software-fullscreen-allowed.png) |
| 呼出后微软拼音候选、空格选词与中文提交 | RS-03 矩阵延续，本项固定字符验证 | 本项补验通过 | [候选](screenshots/femtovg-ime-after-recall.png)、[提交](screenshots/femtovg-ime-committed.png) |

工具会激活目标窗口，输入之间也有调度等待；本项只证明功能与实际输入成功，不从日志时间计算冷启动/热呼出预算或 P95。连续消息测试验证序列处理，不能称为真实键盘高频连按的延迟分布。Software 没有把 redraw requested 当作呈现帧。PNG 由工具按窗口/关联弹窗输出，通常为 640 × 540，不能用它的像素尺寸代替原生客户区/DPI 记录。

## 重现入口与后续边界

```powershell
# 从仓库根目录构建并检查两条路径；先关闭占位热键的夹具。
./scripts/verify-rust.ps1 -SmokeTest
./scripts/verify.ps1 -SkipBuild

# 正式生命周期，不要加 --isolated：
./rust/target/artifacts/software/velo-slint.exe --background --measurement-log=.reasonix/new-rs05.log
# 另一个进程占用 Alt+Space 时会显示回退窗口；可指定其他键。
./rust/target/artifacts/femtovg/velo-slint.exe --hotkey=Ctrl+Alt+K

# 可选夹具：初始化锁定工具链后，在 rust/ 中构建。
. ./scripts/lib/rust-environment.ps1
$rustEnvironment = Initialize-VeloRustEnvironment
Push-Location $rustEnvironment.RustRoot
cargo build --release --locked --no-default-features --features renderer-software --example rs05_desktop_fixture
Pop-Location
./rust/target/x86_64-pc-windows-msvc/release/examples/rs05_desktop_fixture.exe --fullscreen
```

夹具还支持 `--owner=十六进制HWND`、`--reserve-alt-space` 与 `--reserve-f23`，不注入键盘、不执行 Shell、不访问产品数据；普通/全屏夹具按 Esc 退出。所属弹窗由桌面工具作为父窗口关联截图返回；本轮只验证 Win32 owner 边界，未把跨进程测试夹具的销毁当成完整原生对话框关闭/焦点恢复验收。

RS-04 采集流程增加明确隔离边界：`--isolated` 不注册全局键、失焦不隐藏、关闭退出；冒烟与 measurement-stdio 自动隔离。输入采集脚本新增 `--isolated`，继续“输入 → 关闭 → Snapshot”，共享回归通过。单独 measurement-log 保留正式生命周期，用于本项实机证据。旧 RS-04 原始证据没有变更。

RS-06 继续托盘、单实例、顶部拖动、动态高度、运行中 DPI 与多显示器；RS-07 再完成真实原生菜单/文件对话框、导入与系统确认框的临时焦点边界及 G2。Windows 10、其他 Windows 11 版本、第三方 IME、混合 DPI 和游戏独占全屏仍按实际环境补测。快捷键持久化、产品设置草稿/后台业务任务在对应迁移项验证；本项仅保留现有原型状态，没有用不存在的业务任务作为通过依据。
