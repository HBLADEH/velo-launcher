# Rust + Slint 验证工程

这是 [Rust + Slint 重构计划](../docs/rust-slint-refactor-plan.md)的独立 Windows x64 工程。RS-07 已完成本机 Windows 11 x64 的文件拖放、会话内导入预览及所属原生文件选择器/菜单/确认框验证，证据与 Windows 回归矩阵见 [RS-07](../docs/baselines/2026-10-29-slint-rs07/README.md)；两种单次多文件和普通/管理员权限场景见 [2026-10-30 协助补验](../docs/baselines/2026-10-29-slint-rs07/followup-2026-10-30/README.md)。RS-06 的 Win32 托盘、单实例、顶部拖动、动态尺寸与显示环境恢复已实现；本机单屏实测及剩余多屏/混合 DPI 等验收项见 [RS-06](../docs/baselines/2026-10-28-slint-rs06/README.md)，G2 尚未放行。RS-05 的生命周期、热键及冲突恢复见 [RS-05](../docs/baselines/2026-10-28-slint-rs05/README.md)。RS-04 / G1 的固定原型可见 PrivateBytes 中位数为 Software 6.82 MiB / FemtoVG 137.21 MiB，只有 Software 达到 30 MiB 初始预算；这是 RS-04 构建的数据，最终渲染器未决。首轮 A/B、构建身份及测量限制见 [RS-04](../docs/baselines/2026-10-28-slint-rs04/README.md)。RS-03 的本机微软拼音与真实系统 100%/125%/150%/200% DPI 证据继续有效。版本 `0.1.0` 表示验证程序版本，现有 Velo 产品版本仍为 `0.9.11`。

另一台双屏电脑可按 [跨机器复验清单](../docs/rust-slint-dual-monitor-testing.md)拉取分支、构建两种渲染器，并记录 RS-06 的多屏/混合 DPI、拔插/恢复、实际托盘入口和清理。Git 不包含本地 exe，需要先安装下表锁定的工具链。

正常模式为无边框置顶窗口，主页和外观设置使用 640 × 660 逻辑像素，列表高度沿用 Vue 的 `156 + max(1, 结果数) × 54`，限制在 160–720；整个窗口再约束到当前显示器的物理工作区。空结果为 210 高，内容放不下时滚动内层区域；外观面板也能滚动。顶部空白区域可拖动，输入框的选择/编辑和按钮保留原操作，拖动位置在当前会话的显隐及尺寸变化后保留。隔离与旧测量模式继续固定 640 × 540，避免改写 RS-04 的采样条件。页面包括搜索框、六列分组首页、搜索列表、深浅主题、选中/悬停/空结果和减少动画开关。28 个合成条目分为 3 个固定项、12 个常用项和 13 个系统快捷，图标与 Velo 标志嵌入 exe。

输入只按固定数据的名称/路径/说明做简单大小写无关子串过滤，不代表产品搜索算法。首页四方向键遵循 Vue 的跨分组、不完整行和循环规则，列表上下循环、左右保留光标编辑，选中项保持可见。点击或 Enter 只记录选择，底栏显示“已选择”，不调用 Shell 或执行系统操作；Space 是普通输入。IME 合成期间由输入法处理 Enter/Space/方向键/Esc；原生合成状态与预编辑文本共同保护应用命令。

顶栏齿轮打开外观与关于面板；Esc 优先关闭面板，其次清空查询，空查询时隐藏。Alt+Space 显隐；关闭窗口（包括 Alt+F4）隐藏，托盘菜单及底栏“退出”结束进程并释放热键、托盘和实例锁。托盘左键呼出，右键有打开/设置/退出；菜单临时获得焦点不会隐藏主窗口，取消后按实际前台归属恢复焦点或隐藏。失焦到无关窗口时隐藏，所属弹窗不触发这一转换。每次主页呼出恢复原生及搜索输入焦点，显隐保留查询、面板、主题和快捷键草稿。RS-07 已完成本机普通权限的原生文件选择器、右键菜单与确认框，以及普通 Explorer 到管理员目标的阻止边界、管理员同权限 OLE、选择文件回退和返回输入；其他实机边界见 RS-06 与 RS-07 的适用范围。

`--background` 等原生窗口、固定模型及热键注册就绪后保持隐藏；热键注册失败则显示窗口和提示，托盘可用时仍可关闭隐藏并再次呼出；热键和托盘都不可用时不允许隐藏，关闭可以退出。面板支持输入或录制快捷键、应用/重绑及全屏禁用开关；录制期间按当前已注册热键只填入草稿，不会隐藏。先取得新注册再释放旧注册，冲突时旧热键仍有效。设置仅在当前进程保留，正式配置持久化归 RS-08。

正常模式在 Slint 初始化前取得 Windows 会话内的命名实例锁和数据目录锁；再次启动通过原生消息唤起正确实例，不读取业务文件。默认实例为 `prototype`，保留目录为 `%LOCALAPPDATA%\VeloSlintPrototype`，当前目录只参与隔离标识，不创建或读写业务文件。测试副本必须同时指定 `--instance-id=ID --data-dir=PATH`（ID 为 1–48 个英文字母/数字/`-`/`_`）；不同 ID 不能共享同一目录，禁止指向 Go 的 `%LOCALAPPDATA%\Velo` 及其子目录，已有路径的 junction/symlink 会解析后检查。Go/Rust 因而不会同时写原产品数据，正式迁移仍需 RS-08 的共享数据协议。

当前没有业务扫描或业务数据访问；仅对用户拖入/选择/粘贴的文件进行后台元数据检查。底栏“导入预览”进入 640 × 660 页面，可多选文件、粘贴完整路径并查看无效项；普通 `.exe` / `.lnk` 文件存在时标为支持，快捷方式目标尚未解析。导入页失焦保持显示，Esc 返回原查询，主动隐藏结束导入保持模式；预览和路径草稿保留在当前进程。顶部空白处右键或“更多…”打开原生菜单，清空先弹出默认取消的确认框。此阶段不执行文件或保存应用库，正式导入仍按后续业务迁移验收。Go 版若已注册 Alt+Space，Rust 会报告冲突；可用 `--hotkey=Ctrl+Alt+K` 独立验证。`--isolated` 供对照和采集使用：无托盘、实例锁、全局热键或失焦隐藏，关闭直接结束进程。两种冒烟及 measurement-stdio 自动使用隔离模式，不能与 `--background` 混用。

## 已锁定的构建环境

| 项目 | 版本/选择 | 锁定位置 |
| --- | --- | --- |
| Rust / Cargo | 1.99.1，edition 2024 | `rust-toolchain.toml`、`Cargo.toml` |
| Rust target | `x86_64-pc-windows-msvc` | `.cargo/config.toml`、`rust-toolchain.toml` |
| MSVC tools | 14.50.35717；cl 19.50.35719.0 / link 14.50.35719.0 | `windows-toolchain.json` |
| Windows SDK | 10.0.26100.0 | `windows-toolchain.json` |
| Slint / slint-build | 精确 1.18.1 | `Cargo.toml`、`Cargo.lock` |
| winit / raw-window-handle | 0.30.13 / 0.6.2 | `Cargo.lock` |
| Release | opt-level 3、thin LTO、codegen-units 1、panic abort、strip debuginfo | `Cargo.toml` |
| CRT | `+crt-static`，不打包 CJK 字体 | `.cargo/config.toml` / Windows 字体 |

先安装 [rustup](https://rustup.rs/) 以及带 C++ x64 工具的 Visual Studio / Build Tools、上述 MSVC 和 SDK。当前机器复用 Visual Studio Community 2026 的已有 C++/SDK；没有重装或升级 Visual Studio。

```powershell
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
# 以下在仓库根目录执行，默认检查并构建两种渲染器：
./scripts/verify-rust.ps1 -SmokeTest
# 单独构建软件渲染 Release（跳过 fmt/clippy/test）：
./scripts/verify-rust.ps1 -Renderer Software -SkipChecks
```

脚本查找已安装的匹配 MSVC，显式选择 SDK 与 x64 链接器，并核对 compiler/linker 文件版本；缺少版本时失败，不使用最新版代替。rustup 未在 PATH 中时，脚本也会检查 `%USERPROFILE%\.cargo\bin`。工具链安装和依赖首次下载需要联网；使用代理的环境应先按本机配置设置 `HTTPS_PROXY` / `HTTP_PROXY`，rustup/Cargo 不一定读取 Windows 系统代理。

直接运行 Cargo 时应先进入 `rust/`；在仓库根目录仅传 `--manifest-path` 无法保证加载这里的 Rust 工具链和 `.cargo/config.toml`。正式验证使用上面的 PowerShell 入口，以同时锁定原生工具。脚本拒绝覆盖 Rust flags/target 的环境变量。

## Features 与渲染器身份

Slint 禁用 default features，共有 features 为 `std`、`compat-1-18`、`backend-winit`、`unstable-winit-030`、`raw-window-handle-06`。构建时只增加一个：

- `renderer-femtovg` → `slint/renderer-femtovg`，使用 OpenGL；
- `renderer-software` → `slint/renderer-software`，使用 softbuffer 呈现。

必须恰好启用一种渲染器；未选或同时选择均有编译保护。没有默认渲染器，RS-04 后继续比较。没有启用 Qt、Skia、WGPU、Slint 的 system-tray 或 accessibility 功能；托盘直接使用 windows-sys 的 Shell API。`slint-build` 使用其 1.18.1 的默认 `compat-1-18` / `renderer-software` features，后者属于宿主 UI 编译器功能，不会为 FemtoVG 应用启用软件渲染后端。lockfile 中存在可选包也不代表它们被链接；以本次 features 和实际依赖图为准。

`BackendSelector` 在代码中指定 winit 和编译选择的 renderer，不接受环境变量替换。Slint 1.18.1 内部存在渲染器回退路径，因此这里同时采用单渲染器构建，并检查 Slint 与 winit 后端的 feature 图，阻止另一渲染器混入。FemtoVG 还必须观察到 `AfterRendering + NativeOpenGL`。Software 以独占后端 feature、原生重绘事件与非空白 snapshot 共同证明渲染；不把 UI 上的名称单独作为证据。

构建产物分别保存在：

```text
rust/target/artifacts/femtovg/velo-slint.exe
rust/target/artifacts/software/velo-slint.exe
```

每个目录另有 `build-identity.json`、`cargo-metadata.json`、`cargo-tree.txt`、`dependency-licenses.json`。身份包含 exe SHA-256/字节数、工具链、features、Git HEAD/dirty 标记和 `src/`、`ui/` 全部模块/资源的指纹；dirty HEAD 不能单独当作可执行文件的源码身份。整个 `target/` 忽略，不提交本地产物和日志。

## 运行与冒烟检查

```powershell
# 开启事件诊断，直接操作可见窗口：
./rust/target/artifacts/software/velo-slint.exe --diagnostics
# 后台驻留，Alt+Space 呼出；冲突时显示窗口：
./rust/target/artifacts/software/velo-slint.exe --background
# 当前进程使用另一热键：
./rust/target/artifacts/software/velo-slint.exe --hotkey=Ctrl+Alt+K
# 与 Go 对照时禁用正式生命周期，关闭直接退出：
./rust/target/artifacts/software/velo-slint.exe --isolated
# 深色、减少动画、无固定分组：
./rust/target/artifacts/software/velo-slint.exe --theme=dark --reduce-motion --empty-pinned
# 直接打开固定搜索列表；查询不会写入用户数据：
./rust/target/artifacts/femtovg/velo-slint.exe --query=code
# 每次创建实际窗口，2 秒后检查并退出：
./rust/target/artifacts/femtovg/velo-slint.exe --smoke-test --diagnostics
# 独立执行真实 Slint 组件的合成按键/焦点断言，仍在 2 秒后退出：
./rust/target/artifacts/software/velo-slint.exe --input-smoke-test --diagnostics
```

`--smoke-test` 要求 HWND、原生重绘事件、640 × 540 逻辑客户区以及有不同像素的 snapshot；FemtoVG 额外要求 OpenGL 帧。失败或提前关闭返回非零退出码，脚本还设有 15 秒超时。它不测试真实按键、IME 候选框、全 DPI 兼容或内存预算；截图会触发额外渲染，不能在性能测量期间使用此模式。

`--input-smoke-test` 使用默认固定数据，通过真实 Slint 组件发送合成按键，验证 20 项焦点、导航、合成命令保护、面板返回和空结果断言；会改变查询和减少动画状态。`verify-rust.ps1 -SmokeTest` 分别运行窗口冒烟和这一独立输入冒烟。两种冒烟都不能替代微软拼音的实际合成输入记录。

`--diagnostics` 记录 IME 事件类型/合成状态、窗口/输入焦点、生命周期、热键注册结果、编辑后的 UTF-8 字节数、固定过滤结果数量、固定导航 ID 与拖入数量，不输出输入内容或文件路径；不会关闭正式失焦隐藏。选择日志仅含固定样例 ID 和 `action=record_only`。拖放进入会话内预览，只检查文件类型、存在性和是否为文件，不读取内容或执行文件；权限、实机证据与待验项见 RS-07。

## RS-04 资源与输入采集

`--measurement-log=PATH` 开启独立、不可覆盖的测量日志，记录原生 HWND、重绘请求、可观察的 OpenGL 帧、原生/组件焦点和真实输入回调；不保存查询内容。写入使用容量 128 的后台队列，丢失事件会使采集记录失效。不能与 `--query` 或冒烟探针混用，避免把预设字符串当成实际输入。

`--measurement-stdio` 要求同时提供日志路径，通过继承的标准输入接收 `status`、`hide`、`show`、`warm-bottom`、`warm-top`、`clear` 和 `quit`。控制线程阻塞等待，没有重复计时器或监听端口；父采集器断开时退出。它自动启用隔离模式，保留同一个原生窗口并关闭隐藏时的 presented 动画状态。普通运行只有阻塞等待原生消息的热键线程；测量写入/控制线程仅在请求时建立。

单独使用 `--measurement-log=PATH` 可以观察正式生命周期；输入延迟采集脚本显式添加 `--isolated`，继续使用“输入 → 关闭 → Snapshot”流程。正式模式的关闭会隐藏，需要使用底栏“退出”结束采集。

```powershell
# 完整首轮资源采样：实际预热全部首页行，三种状态各 3 × 15 秒，100 次诊断显隐后复采。
# 输出目录必须不存在；每个渲染器需要数分钟，不在此期间截图或构建。
./scripts/measure-slint-ab.ps1 -Renderer software -OutputDirectory .reasonix/my-software-ab
# 只补采可见状态，可用于反转 A/B 顺序。
./scripts/measure-slint-ab.ps1 -Renderer femtovg -OutputDirectory .reasonix/my-femtovg-visible -VisibleOnly

# 输入观察：Arm 一次，重复 Capture → 实际输入 → 正常关闭 → Snapshot。
./scripts/collect-latency.ps1 -ApplicationKind Slint -Action Arm -WorkDirectory .reasonix/my-input -OutputDirectory .reasonix/my-input -Executable rust/target/artifacts/software/velo-slint.exe
./scripts/collect-latency.ps1 -ApplicationKind Slint -Action Capture -WorkDirectory .reasonix/my-input -OutputDirectory .reasonix/my-input
# 此时用真实键盘或 Computer Use 输入字符，正常关闭窗口后：
./scripts/collect-latency.ps1 -ApplicationKind Slint -Action Snapshot -WorkDirectory .reasonix/my-input -OutputDirectory .reasonix/my-input
```

共享资源脚本使用 `-ApplicationKind Slint -ProcessId <PID> -WindowHandle <实测十六进制HWND>`，读取产物旁的 `build-identity.json`，不会对 Rust exe 执行 `go version -m` 或读取产品数据目录。增加每秒原始记录、私有工作集、线程/句柄及可用的 GPU 计数；null 表示不可采，不能按零统计。默认 ApplicationKind 仍为 Wails，旧采集入口继续可用。

输入记录以 TextInput 真正接收字符为结束边界，保存失败观察，复用原有延迟汇总。Computer Use 会激活窗口且有调度/操作等待，因此当前方法 `ComputerUseAssistedUpperBound` 只能证明观察到输入，不能据此宣称暖启动或热呼出预算达标。Software 没有可用的首帧 notifier，`first_redraw_requested` 只表示请求，不标为已呈现帧。采集检查可单独执行 `./scripts/test-slint-measurement.ps1`。

Windows 接入方式见 [RS-01 基线](../docs/baselines/2026-10-28-slint-rs01/README.md)，固定界面见 [RS-02 基线](../docs/baselines/2026-10-28-slint-rs02/README.md)，输入/动画与实际微软拼音记录见 [RS-03 基线](../docs/baselines/2026-10-28-slint-rs03/README.md)。`cargo test` 包含 26 项固定数据、导航、合成输入、生命周期、热键解析、真实 Windows 注册/冲突/重绑/释放、托盘端点/恢复/清理、实例隔离、尺寸/负坐标及导入预览/原生多选路径/迟到结果边界检查；消息注入与纯状态测试不能代替实际按键、多屏或 Shell 点击记录，产品共享数据契约仍按后续计划实现。

生命周期状态与 UI 适配位于 `src/lifecycle.rs`，Win32 热键/窗口边界位于 `src/platform/windows/`。`examples/rs05_desktop_fixture.rs` 是无产品数据的临时前台窗口，可用 `--fullscreen`、`--owner=十六进制HWND`、`--reserve-alt-space` 或 `--reserve-f23` 验证全屏、所属弹窗及冲突，Esc 退出。可在初始化构建环境后用 `cargo build --release --locked --no-default-features --features renderer-software --example rs05_desktop_fixture` 构建；详细步骤见 RS-05 记录。

RS-06 的 `desktop.rs` 在独立阻塞消息线程上管理托盘及实例唤起，`instance.rs` 管理初始化前的双锁与路径隔离，`geometry.rs` 使用有符号物理坐标、显示器工作区及原生 DPI 放置窗口。显示设置、DPI 和恢复事件回到 Slint 事件循环后合并处理；拖动期间延后约束，避免跨屏时被拉回旧屏。Explorer 重启后重新注册图标，通知区未就绪时最多重试十次，没有永久轮询。`examples/rs06_native_probe.rs` 无参数读取显示器、客户窗口和托盘注册状态；`--tray-left ID`、`--tray-menu ID`、`--taskbar-created ID`、`--environment ID` 与 `--move HWND X Y` 用于明确标记的原生消息/几何注入，不能替代实际指针、物理拔屏或休眠验收。构建方法与前一个 example 相同，替换 example 名即可。

托盘的 `ui/assets/velo.ico` 原样复制自 `build/windows/icon.ico`，包含 16/32/48/64/128/256px 图层，由 Rust 的 `include_bytes!` 嵌入。按通知区域所在显示器的 DPI 请求小图标尺寸（本机实测 16/20/24/32px），选择不小于目标的源图层后交给 Win32 解码缩放；替换或退出时销毁应用持有的旧 HICON。RS-02 的其他固定资源仍通过 Slint 编译器嵌入。

界面适配位于 `src/ui.rs`，固定数据位于 `src/fixtures.rs`，纯导航与输入规则位于 `src/navigation.rs` / `src/input.rs`，可选组件输入探针位于 `src/interaction_probe.rs`；主题与共用组件位于 `ui/components.slint`，页面位于 `ui/app.slint`。滑动使用 Slint Flickable 与固定宽度滚动条，不使用 Fluent 内置不可关闭的滚动条/复选框过渡。首页/列表有 240ms 内容入场、tile 有 360ms 淡入及 24ms 顺序延迟，另有悬停、按压和面板过渡；均为有限动画。减少动画将时长和延迟归零，并直接显示最终透明度/位置/尺寸，未接 Windows 系统偏好。主题与查询仅在当前进程保留。

正常运行使用系统 DPI，已完成 [真实 DPI 补验](../docs/baselines/2026-10-28-slint-rs03/native-dpi/README.md)。`SLINT_SCALE_FACTOR` 仅供开发时渲染比例预验：与原生 DPI 不同时，本机微软拼音候选框出现偏移，不能替代真实系统 DPI 验收；真实 100%/150%/200% 下未复现该偏移。RS-06 已在同一运行中的实例上补验两种渲染器的 100%/125%/150%/200% 单屏 DPI、真实输入、工作区约束及状态保留，并恢复系统 125%；物理多屏/混合 DPI/拔插与休眠恢复仍待实机验证。无前台权限的后台工具父进程启动曾观察到窗口可见但 Windows 未授予激活；真实热键及资源管理器二次启动均验证原生焦点成功，详见 RS-06 的观察记录。

RS-07 的 `src/import_preview.rs` 将文件元数据检查放到容量 2 的按需工作队列，预览最多 256 项；40ms 单次计时器合并原生逐文件拖放事件，清空后拒绝旧 generation 的结果。`platform/windows/interaction.rs` 在至多一个临时 STA 线程上处理有真实 owner 的原生多选、菜单和确认框；UI 线程接收结果并恢复实际前台所属页面的焦点。原生交互期间暂时阻止应用显隐、设置切换和退出；取消后恢复正常行为。提权模式显示普通 Explorer 拖放边界提示并保留选择文件入口，没有放宽系统 UIPI 过滤。完整记录见 [RS-07](../docs/baselines/2026-10-29-slint-rs07/README.md)。

## 许可与发行前要求

本工程自有代码沿用仓库 MIT。Slint 提供多种许可选择，本桌面原型使用 [Slint Royalty-free 2.0](licenses/Slint-Royalty-free-2.0.md)，在顶栏齿轮可访问的“外观与关于”面板保留官方 `AboutSlint` widget。Slint 依赖不受项目 MIT 覆盖。直接依赖 raw-window-handle 0.6.2 与 windows-sys 0.61.2 均为 MIT / Apache-2.0 双许可；嵌入 Fluent SVG 的 MIT 许可与来源见 [资源说明](ui/assets/README.md)。

发行时还需从锁定构建的依赖许可证清单收集完整第三方 notices/许可文本，并验证 Windows 10/11、OpenGL 驱动和系统 DLL 的最低要求。这些属于 RS-14，当前不生成或替换安装包。静态 CRT 也不意味着不依赖 Windows 系统组件。不要将两种渲染器的 exe 大小当成内存结果。
