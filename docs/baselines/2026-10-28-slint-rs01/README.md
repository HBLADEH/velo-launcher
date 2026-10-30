# RS-01：工具链、Windows 接入与最小工程

执行日期：2026-10-28（北京时间）。对应 [Rust + Slint 重构方案](../../rust-slint-refactor-plan.md)的 RS-01。

**结论：RS-01 已验证，可以推进 RS-02。** 已建立独立 Rust 工程、锁定工具链和依赖、完成两种 Release 构建，并查明 Windows 原生窗口、IME 与文件拖放的接入点。两种渲染器都通过真实窗口冒烟和基础中文输入检查。本记录不代表 G1/G2 通过，也不包含内存、启动延迟或完整 IME/DPI 兼容结论。

## 构建身份与产物

- 分支：`codex/rust-slint-refactor`；基于 `ac2f51ad1317f4abffab2c548af251e48ce16d2b` 的 dirty 工作区，无新提交。最终源文件 SHA-256 见两份构建身份，不能用 HEAD 代替本次源码身份。
- 系统：Windows 11 Pro x64，10.0.26300，单次实测缩放 125%。原生日志客户区为 800 × 675 物理像素，即 640 × 540 逻辑像素。
- Rust / Cargo 1.99.1，rustc commit `58cf8f9c35b355199727627d07ae34a4e2846d67`；rustup 1.29.2。复用本机 Visual Studio Community 2026 18.0.2。
- MSVC tools 14.50.35717，cl 文件版本 19.50.35719.0，link 文件版本 14.50.35719.0；Windows SDK 10.0.26100.0。入口脚本严格核对这些版本。
- Slint / slint-build 1.18.1、winit 0.30.13、raw-window-handle 0.6.2。直接依赖使用精确版本，应用提交 `Cargo.lock`。
- 相同 Release 设置：opt-level 3、thin LTO、codegen-units 1、panic abort、strip debuginfo、静态 CRT。相同 winit 事件/窗口后端；仅改变 renderer feature。原生窗口暂用标题栏与固定客户区，产品的无边框、置顶和生命周期按后续编号实现。

| 最终 Release | 字节数 | SHA-256 | 身份 |
| --- | ---: | --- | --- |
| FemtoVG / OpenGL | 9,913,344 | `CF8E64341CF7A74D0CF9C6DE81C2B767D495BAD5F42625FCB314C909184CC9CC` | [femtovg-build.json](femtovg-build.json) |
| Software / softbuffer | 10,417,152 | `3A450389960FF2A9CB14418771C09ED92D7DA55BF9A28424C9A7732ACD64C80C` | [software-build.json](software-build.json) |

产物在 `rust/target/artifacts/{femtovg,software}/velo-slint.exe`，未提交。上述大小只描述文件体积，不是内存占用。两个构建的依赖图、完整 Cargo metadata 与原始日志保留在忽略的 `rust/target/` / `.reasonix/rs01/`；公开 [dependency-licenses.json](dependency-licenses.json) 包含合并后的 315 个第三方包许可声明（含宿主编译器依赖），不是 315 个运行时常驻组件。

手工输入截图采自首轮 Release，其 exe SHA-256 分别为 FemtoVG `548BA3713FA40BCB34A7847210CD4B29725F738C3159B1153459632A8FFEA0AD`、Software `701425AC6B655B2F8E922F56D1E5F8C893EB9A0547E72995D8660BB61A54EA43`。随后加强原生工具链版本检查并重建，得到上表最终产物；两轮 `src/main.rs`、`ui/app.slint`、`build.rs`、`Cargo.toml` 和 `Cargo.lock` 内容一致，最终产物重新通过窗口冒烟。输入截图和最终冒烟分别记录，不混作同一次运行。

## Windows 能力核实

源码依据固定为 Slint crates.io 1.18.1 的 VCS commit `372cf0ee5577c3dfec309a45e7b778ba4e81b734`，以及 winit 0.30.13 的 `e9809ef54b18499bb4f2cac945719ecc2a61061b`。

| 能力 | 本版本接入方式与本次证据 | 边界 / 后续验收 |
| --- | --- | --- |
| 窗口和事件循环 | `BackendSelector::backend_name("winit")`；Slint UI 线程运行事件循环，两个构建均观察到原生重绘 | 只承诺 Windows x64 工程；未实测 Windows 10 |
| HWND | `slint::winit_030::WinitWindowAccessor::with_winit_window` + raw-window-handle 0.6 的 `HasWindowHandle` → `RawWindowHandle::Win32.hwnd`；两份 Release 均取得非零句柄 | accessor 需在原生窗口已创建时调用；这里只短借 winit 窗口，避免持有 Arc 妨碍隐藏；显隐后不能假定 HWND 不变 |
| 窗口属性 / 激活 | winit 提供 `set_decorations`、`set_window_level`、`focus_window`；原型用 Slint 布局约束固定客户区，并调用输入框 focus | 属性接口存在不等于激活/多屏正确；无边框、置顶、失焦与后台启动在 RS-02/RS-05/RS-06 验收 |
| IME | winit `WindowEvent::Ime` → Slint 的 `UpdateComposition` / `CommitComposition`；Slint 调用 `set_ime_allowed` 与 `set_ime_cursor_area`。原型事件过滤返回 `Propagate`，不吞合成事件；两个构建均实测 `n`、`i`、Space → “你”，日志出现 preedit / commit，编辑内容为 3 个 UTF-8 字节 | 当前中文 IME；不把文字注入当作拼音输入证据。候选翻页、Enter/方向键/Esc 的完整合成保护、第三方输入法及不同 DPI 仍待 RS-03 |
| 文件拖放 | 同一个 `on_winit_window_event` 观察 `WindowEvent::DroppedFile(PathBuf)`；Windows winit 默认启用 drag-and-drop。原型只更新计数，不读取文件、不调用 Shell | 本次完成版本源码核实和接口编译，未进行 Explorer 真拖入、文件类型、多文件、中文路径或提权实测；RS-07 验收 |
| FemtoVG | 只启用 Slint/winit 后端 `renderer-femtovg`；`AfterRendering` 实际报告 `NativeOpenGL`，并有非空白 snapshot | 本机驱动已运行；驱动兼容、显存及效果成本待 RS-04/RS-12 |
| Software | 只启用 Slint/winit 后端 `renderer-software`，使用 softbuffer；原生重绘和非空白 snapshot 通过，实机文字与候选框可见 | 不伪造 OpenGL notifier；复杂阴影、动画、高 DPI 成本待 RS-02/RS-04 |

关键源码与官方 API：

- [Slint 1.18.1 BackendSelector](https://docs.rs/slint/1.18.1/slint/struct.BackendSelector.html)、[winit_030 集成模块](https://docs.rs/slint/1.18.1/slint/winit_030/index.html)。
- [固定版本 accessor / renderer 创建与回退](https://github.com/slint-ui/slint/blob/372cf0ee5577c3dfec309a45e7b778ba4e81b734/internal/backends/winit/lib.rs)、[IME 转发与候选框位置](https://github.com/slint-ui/slint/blob/372cf0ee5577c3dfec309a45e7b778ba4e81b734/internal/backends/winit/winitwindowadapter.rs)。
- [winit Windows 窗口属性](https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/src/platform/windows.rs)、[Windows 默认拖放设置](https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/src/platform_impl/windows/mod.rs)、[Windows IME 事件处理](https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/src/platform_impl/windows/event_loop.rs)。

`unstable-winit-030` 是显式启用的不稳定集成接口，升级 Slint / winit 时需复查。Slint 默认 features 被禁用；使用 `compat-1-18` 避免旧兼容 feature 自动拉入软件渲染器。两份构建的 winit 后端还包含上游 `default/muda/x11/wayland/raw-window-handle-06` features，详见身份 JSON；x11/wayland 受目标平台条件限制，不代表本次支持 Linux。

Slint 1.18.1 的 `BackendSelector` 并不能单独保证绝不回退，底层创建 renderer 时存在默认与回退分支。因此检查 Slint 和 winit 后端都只能启用一项 renderer feature，再以实际帧、HWND 和 snapshot 校验。另一种渲染器无法在该二进制中接管样本；启动失败保留失败，不给它标记成功身份。

## 验证结果与截图

| 检查 | FemtoVG | Software |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 通过（共用源码） | 通过（共用源码） |
| clippy `--all-targets -- -D warnings` | 通过 | 通过 |
| `cargo test --locked` + 对应独占 feature | 通过，0 个业务测试 | 通过，0 个业务测试 |
| Release、锁定依赖 / features 检查 | 通过 | 通过 |
| HWND / redraw / 非空白 snapshot | 通过；最终运行 8 次 redraw | 通过；最终运行 8 次 redraw |
| 实际 renderer | 观察到 NativeOpenGL 完成帧 | 独占 Software feature + 实际内容渲染 |
| 激活窗口后不点击输入框，直接输入拼音 | `ni` + Space 提交“你” | `ni` + Space 提交“你” |
| 中英文混合文本显示 | 通过 | 通过 |
| 普通关闭退出 | 通过，未遗留测试进程 | 通过，未遗留测试进程 |

实机输入使用 computer-use 技能，通过窗口激活与实际按键完成拼音检查，再用文本注入补充中英混排检查；窗口激活步骤的存在意味着这还不能证明无人干预首次启动的前台抢焦点行为。默认关闭 accessibility feature，当前 UI Automation 仅能看到窗口/标题栏，后续若需要读屏支持必须单独开启并验证。

| 场景 | FemtoVG | Software |
| --- | --- | --- |
| 拼音合成与真实候选框 | [截图](femtovg-ime-preedit.png) | [截图](software-ime-preedit.png) |
| 中文提交后追加中英混排文本 | [截图](femtovg-input.png) | [截图](software-input.png) |

另已验证：不选择 renderer 的 `cargo check` 被拒绝；同时选择两项 renderer 的 `cargo check` 被应用编译保护拒绝。`dumpbin /dependents` 未显示 VC/UCRT redistributable DLL 依赖；两份仍依赖 Windows 系统 DLL，FemtoVG 另有 `opengl32.dll`。尚未在干净 Windows 10/11 虚拟机上做发行验证。

验证入口：

```powershell
./scripts/verify-rust.ps1 -SmokeTest
```

最终加固后执行 `./scripts/verify-rust.ps1 -SkipChecks -SmokeTest`，重新核对工具版本、依赖图、源文件指纹和两份实际窗口；没有修改 Rust 源码，因此不重复前一轮已通过的 clippy/test。PowerShell 两个新增脚本的语法检查及 `git diff --check` 通过。没有修改 Go/Vue 或共享采集工具，本项未运行 Go/Wails 打包与旧版整套回归。

## 许可与下一步

本项目自有代码继续使用 MIT；Slint 桌面原型选择 [Royalty-free 2.0](../../../rust/licenses/Slint-Royalty-free-2.0.md)，窗口实际展示官方 AboutSlint widget，保留其许可文本。其他依赖的声明已形成公开清单；完整第三方 notices 打包与发行条件仍在 RS-14。

RS-02 从本工程继续实现固定样例图标、六列首页/列表、深浅主题和选中状态；同时准备减少动画开关。RS-03 再做完整输入/导航/IME 保护，RS-04 适配采集入口并首次测内存和延迟。本次没有用小窗口或 exe 大小推断内存预算达标。
