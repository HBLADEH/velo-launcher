# RS-06：托盘、单实例、拖动与窗口几何

执行日期：2026-10-28 至 2026-10-29（北京时间，跨日收尾；目录沿用开始日期）。对应 [Rust + Slint 重构方案](../../rust-slint-refactor-plan.md)的 RS-06。

**状态：待验证。代码实现、本机单屏交互和两条渲染路径的自动检查已完成；物理多屏/混合 DPI/拔插、休眠恢复及 Shell 通知区域图标的实际左右点击仍待补验。G2 未放行，未开始产品业务或数据迁移。** 托盘菜单本身使用真实指针选择，其入口由明确标记的原生消息探针触发，不能当成实际点击了系统托盘图标。

## 构建与范围

沿用 `codex/rust-slint-refactor` 的已有 dirty 工作区，HEAD 为 `ac2f51ad1317f4abffab2c548af251e48ce16d2b`。没有提交或覆盖现有 Go/Wails 用户修改，没有替换产品安装包。Rust/Cargo 1.99.1、MSVC 14.50.35717、SDK 10.0.26100.0、Slint 1.18.1、静态 CRT 与两条独占 renderer features 保持原约定。只增加现有 `windows-sys 0.61.2` 的 Security/HiDpi/Shell features，没有新增第三方 crate。

| 渲染器 | exe 字节数 | SHA-256 | 身份 |
| --- | ---: | --- | --- |
| Software | 10,712,064 | `08AA458E8E6F65AEAA8C253701C3D5F6E0509A9FDF04CEA11A34BEAA96BD018B` | [software-build.json](software-build.json) |
| FemtoVG | 10,210,304 | `E7E1BBB2E2DD05BB89C33001302DAD9C282D0FF11459598D66CBDA568D5D11F3` | [femtovg-build.json](femtovg-build.json) |

两份身份各含 47 个源码/脚本/资源指纹，含新原生模块、探针及 ICO；收尾已逐文件核对与当前源码和实测 exe 一致。`rust/README.md` 与本记录是说明文档，未重建或改标实测 exe。探索构建、原始本地日志保留在忽略目录 `.reasonix/rs06/`；公开记录包含日志 SHA 与脱敏事件，不提交程序或产品数据。

系统为同一台 Windows 11 x64，只有一块 2560 × 1440 显示器。实测通过系统显示设置更改真实 DPI，没有使用 `SLINT_SCALE_FACTOR`。最后恢复原来的 125%（120 DPI）、2560 × 1440 与底部任务栏，原生工作区为 `(0, 0, 2560, 1380)`。本项不重采内存、启动或热呼出延迟；RS-04 的 PrivateBytes 结果只属于原 RS-04 构建，最终渲染器仍未决定。

## 实现

- [desktop.rs](../../../rust/src/platform/windows/desktop.rs)用一个阻塞于 `GetMessageW` 的 Win32 消息线程管理隐藏顶层服务窗口、托盘图标、原生菜单和实例激活端点。回调排入 Slint UI 事件循环，不在工作线程操作组件。菜单有打开/设置/退出，左键通知调用打开；菜单活动状态和真实 owner 关系参与失焦判断，退出销毁菜单/图标/窗口并 join 线程。
- 托盘使用原样复制自 `build/windows/icon.ico` 的多分辨率 `ui/assets/velo.ico`，由 Rust 嵌入。按通知区域实际显示器的 DPI 选择源图层并创建 HICON，本机日志观察到 16/20/24/32px；替换时销毁旧句柄。`TaskbarCreated` 后重加图标，Explorer 尚未就绪时最多重试十次；正常空闲没有重复恢复计时器。没有把 HICON 尺寸日志当成图标清晰度的视觉验收。
- [instance.rs](../../../rust/src/platform/windows/instance.rs)在 Slint 和测量文件初始化前取得会话内的实例锁与数据目录锁。再次启动向对应 ID 的服务窗口发送有界超时消息，并在允许时转交前台权限；端点尚未建立时最多等待四秒，失败不会启动竞争界面。RAII 关闭锁句柄。
- 默认实例为 `prototype`，预留数据目录为 `%LOCALAPPDATA%\VeloSlintPrototype`。测试副本必须同时指定 `--instance-id` 和 `--data-dir`；不同 ID 不能共享目录，路径会处理大小写、`..` 及已有祖先的 junction/symlink。拒绝 Go 的 `%LOCALAPPDATA%\Velo` 及其子目录。当前只把目录用于隔离锁命名，不创建、读写或迁移业务文件。
- [geometry.rs](../../../rust/src/platform/windows/geometry.rs)区分逻辑内容尺寸和有符号物理坐标。主页/外观设置为 640 × 660；列表沿用 Vue 的 `156 + max(1, 结果数) × 54` 并限制在 160–720。0/1 个结果为 210 高、2 个为 264、8 个为 588、大量结果最高 720；再按实际 DPI 转换并约束到显示器工作区。导入页的 660 规则已有纯布局用例，实际导入 UI 和模式按 RS-07 接入，未冒充本项已有导入页。
- [lifecycle.rs](../../../rust/src/lifecycle.rs)在首次显示时跟随指针所在显示器；后续显隐、内容尺寸和 DPI 变化保留会话位置，离屏时约束到最近显示器。布局在 winit/Slint 消费原生事件后合并执行；系统移动循环期间延后约束，避免跨屏拖动被拉回旧屏。显示/工作区设置和恢复事件回到 UI 线程处理，迟到回调不显示隐藏窗口。
- [app.slint](../../../rust/ui/app.slint)的顶部空白区域调用 winit 原生拖动，搜索编辑和按钮保留自己的输入。空结果适应短窗口，外观面板可在受限高度下滚动。正常关闭保留同一个组件和 HWND；托盘或热键至少一条可恢复时允许隐藏，两者都失败时保留可访问窗口。隔离/冒烟/旧测量模式继续固定 640 × 540，不创建托盘或实例锁。

## 自动检查

完整摘要及原日志指纹见 [checks.json](checks.json)。

| 检查 | 结果与证据边界 |
| --- | --- |
| `./scripts/verify-rust.ps1 -SmokeTest` | fmt、两种 feature 的 clippy `-D warnings`、各 21 项测试、两份 Release、4 次窗口/输入冒烟全部通过；每条输入冒烟各 15 项组件断言 |
| 新增 Windows/布局覆盖 | 真实命名 mutex、同目录拒绝、释放后重新取得；实例端点转发；托盘注册、注入 `TaskbarCreated` 后恢复及 Drop 后端点销毁；Vue 尺寸契约、负原点、小工作区、移除显示器后的几何及托盘恢复路径 |
| `./scripts/verify.ps1 -SkipBuild` | 共享采集回归、Go vet/test、前端 lint/类型检查、26 项前端测试及构建通过 |
| 正常模式 CLI 拒绝路径 | 缺少配对 data-dir、另一 ID 共享正在使用的目录、指向 Go 产品目录，三个独立进程均在 UI 初始化前以退出码 1 拒绝；见 [native-observations.json](native-observations.json) |
| 身份和事件完整性 | 每份 47 个指纹一致；Software 171 / FemtoVG 92 条测量事件，全部 `dropped=0`；每次运行仅一次 native-window 事件，同一 HWND 保留到退出 |
| 退出清理 | 两次真实菜单退出后原生端点和测试进程均不存在；另一进程成功注册 Alt+Space 后退出；见 [cleanup.json](cleanup.json) |

合成按键、消息注入与纯几何测试只覆盖各自边界，不替代微软拼音、多屏、物理拔插或实际 Shell 指针输入。RS-03 的真实 IME 记录延续，本项不重新宣称完整 IME 矩阵。

## 本机实测矩阵

[desktop-events.json](desktop-events.json)保存两次生产模式运行的测量/诊断事件及原日志 SHA；[native-observations.json](native-observations.json)保存 14 份原生显示器/窗口/托盘状态快照、实例保护检查、入口注入标记和 Explorer 进程变化；[screenshots.json](screenshots.json)保存 21 张截图的尺寸和 SHA。Software 为 PID 2964 / HWND `8A09E2`，FemtoVG 为 PID 30252 / HWND `20608`。

| 场景 | 结果 | 证据 |
| --- | --- | --- |
| 正常主页、搜索及空结果动态高度 | 两条路径参与：主页/设置 660，2 个结果 264，空结果 210，大量结果 720；实际字符进入组件，Software 4 次、FemtoVG 2 次非合成输入事件同时满足组件/原生焦点与可见性 | [Software 首页](screenshots/software-home-125.png)、[264](screenshots/software-search-264.png)、[720](screenshots/software-search-max-720.png)、[FemtoVG 空结果](screenshots/femtovg-empty-210.png) |
| 顶部拖动 | 两种渲染器真实拖动通过；Software 从 `(880,276)` 移到 `(1030,351)`，FemtoVG 最终从 `(800,273)` 移到 `(925,348)`（物理坐标） | [Software](screenshots/software-dragged.png)、[FemtoVG](screenshots/femtovg-dragged-125.png)，原生状态快照 |
| 关闭隐藏与重复启动 | Software 拖动后 Alt+F4 隐藏，工具第二进程退出并转发到原 PID，保留查询、HWND 及位置；随后从资源管理器真实双击 exe，原 PID/查询保留且 `native_focus_sync focused=true` | [真实二次启动](screenshots/software-explorer-second-instance.png)，shown/second_instance 事件 |
| 独立测试副本 | Software 默认实例与 FemtoVG `rs06-femtovg` 同时存活，分别使用独立数据目录与 Alt+Space / Ctrl+Alt+K；FemtoVG `--background` 保持隐藏，实际热键呼出并输入 | [FemtoVG 呼出](screenshots/femtovg-home-150.png)、两组 PID/端点快照 |
| 单屏运行中 DPI | Software 同一实例经过 125→100→150→200→100→125；FemtoVG 从 150 启动后经过 200→100→125。均保持 HWND/查询，真实热键呼出并观察输入，原生 `ScaleFactorChanged` 和 DPI/工作区日志齐备 | [Software 100](screenshots/software-live-dpi-100.png)、[150](screenshots/software-live-dpi-150.png)、[FemtoVG 100](screenshots/femtovg-live-dpi-100.png)、[200](screenshots/femtovg-live-dpi-200.png) |
| 高 DPI 与任务栏工作区约束 | 200% 时工作区为 2560 × 1344；Software 720 的请求正确限制成 1280 × 1344 物理像素，即 640 × 672 逻辑像素，列表可滚动；恢复 125% 后重新达到 720 | [受限窗口](screenshots/software-live-dpi-200-clamped.png)、两组 200%/恢复 125% 原生快照 |
| 离屏位置恢复 | 把 Software 程序化移到 `(-4000,-3000)` 后，原生移动事件使它回到 `(0,0)` 工作区并保留设置页；这是离屏注入，未冒充物理拔屏 | [恢复后](screenshots/software-offscreen-clamped.png)、before/after-offscreen 快照 |
| Explorer 实际重启 | 先关闭本次测试打开的资源管理器窗口，再重启当前会话 Explorer；原 PID/窗口保留。Explorer 从 13500/14112/31828 变为 7912，两图标的 `Shell_NotifyIconGetRect` 均重新可用，日志重新创建 20px 图标 | ActualExplorerRestart 与 after-explorer-restart 快照、两组 tray-icon/desktop-event 记录 |
| 托盘回调呼出 | 两个隐藏实例均经探针注入真实的 Shell 左键通知，回到正确实例并保留页面/查询；只证明通知派发路径，不算实际点击 Shell 图标 | [Software](screenshots/software-tray-recalled-after-explorer.png)、[FemtoVG](screenshots/femtovg-restored-125-tray.png)，InjectedTrayEntries 记录 |
| 原生托盘菜单 | 探针仅触发菜单入口，随后使用真实指针选择打开/设置/退出。菜单前台时 `owned_foreground=true action=None`，没有误隐藏；设置恢复 660，两个渲染器的真实“退出”均结束进程 | [菜单](screenshots/software-native-tray-menu.png)、[设置](screenshots/software-tray-settings-660.png)、[FemtoVG 菜单](screenshots/femtovg-native-tray-menu.png)、两个 desktop-stopped 记录 |
| 清理与系统恢复 | 最终没有 velo-slint/夹具进程或原生端点；Alt+Space 重获/释放成功；恢复 125% 后关闭本次测试的设置与文件资源管理器窗口 | [cleanup.json](cleanup.json) |

截图由 Computer Use 按窗口和关联菜单输出，通常是 640px 宽的逻辑图像；不能从 PNG 尺寸推断物理客户区、DPI 或呈现延迟。没有使用工具等待时间计算延迟预算。

## 尚未通过的边界

1. 本机只有单屏，副屏、左侧/上方负坐标、混合 DPI 跨屏拖动、物理显示器拔插没有实机证据。纯几何用例和离屏注入已通过，物理矩阵仍待验证；真实休眠/恢复也没有执行。
2. 当前 Computer Use 没有提供可直接选中的 Shell 通知区域窗口，因此托盘左右入口采用显式原生消息注入。实际 Shell 图标点击、键盘入口和高 DPI 图标的直接视觉检查仍待补验；真实 Win32 菜单选择和 Explorer 重启已经完成。
3. 无前台权限的工具后台启动、后台二次启动或注入托盘通知时，Software 有 3 次、FemtoVG 有 1 次 `native_focus_sync focused=false`，随后窗口可见但未取得原生输入焦点。这些观察保留在事件记录里，没有用工具随后激活窗口来改判。真实热键、资源管理器双击二次启动和真实菜单项选择都取得原生焦点；任意后台父进程请求激活的边界仍需进一步确认，不能把可见当作可输入。
4. Windows 10、其他 Windows 11 环境、第三方 IME、物理小屏的完整面板操作仍按实际环境补测。完整导入模式、文件拖放、原生文件选择器/系统确认框及临时焦点矩阵属于 RS-07；没有用尚不存在的业务页或任务作通过证据。

因此 RS-06 在总计划中保持 **待验证**。本机可完成的实现与验证已交付，后续先补上述实机边界，再完成 RS-07/G2；不越过 G2 开始大规模业务迁移。

## 2026-10-29 环境复查与暂缓

按用户“看看当前环境是否好验证，不好就先搁置”的指示完成只读复查，结果见 [environment-review-2026-10-29.json](environment-review-2026-10-29.json)。当前交互会话和窗口枚举正常，但仍只有一块 2560 × 1440 / 125% 显示器，工具未返回可定位的 Shell 通知区域窗口。`powercfg /a` 显示 S3 睡眠可用、系统休眠未启用；完整睡眠/恢复观察需要安排唤醒配合，会中断当前交互会话，本次没有触发睡眠或修改电源设置。

当前条件不适合补齐剩余实机矩阵，**按用户指示先搁置这些待验项**。保留此前已通过的实现、自动检查和本机证据，RS-06 仍为待验证，G2 不放行；本次未改源码、系统缩放或设置，也没有启动或遗留测试进程。后续有副屏/混合 DPI 环境、可定位的托盘入口和唤醒配合时，再恢复对应验证。

## 重现入口

```powershell
# 两种 renderer 的检查、Release 与隔离窗口/输入冒烟。
./scripts/verify-rust.ps1 -SmokeTest
./scripts/verify.ps1 -SkipBuild

# 正常实例；再次执行会唤起已有实例。
./rust/target/artifacts/software/velo-slint.exe --diagnostics
# 独立副本必须同时更换实例 ID、目录和冲突的热键。
./rust/target/artifacts/femtovg/velo-slint.exe --background --hotkey=Ctrl+Alt+K --instance-id=rs06-copy --data-dir=D:/velo-rs06-copy

# 原生只读检查/显式注入探针，先初始化锁定的构建环境。
. ./scripts/lib/rust-environment.ps1
$rustEnvironment = Initialize-VeloRustEnvironment
Push-Location $rustEnvironment.RustRoot
cargo build --release --locked --no-default-features --features renderer-software --example rs06_native_probe
Pop-Location
./rust/target/x86_64-pc-windows-msvc/release/examples/rs06_native_probe.exe
# 以下是注入，不是实际 Shell 点击：
./rust/target/x86_64-pc-windows-msvc/release/examples/rs06_native_probe.exe --tray-menu prototype
```

如需测量事件日志，提供新的 `--measurement-log=PATH`，其父目录必须已存在，目标文件不能存在。正式模式关闭只隐藏；测试完通过托盘菜单或底栏明确退出，再检查进程和热键清理。
