# Rust 工程说明

[开发与构建](development.md) · [重构历史](rust-slint-refactor-plan.md)

当前工程是仓库根目录下的 Windows x64 Cargo 包 `velo-slint`，由 Rust + Slint 构成。工具链与依赖版本保持锁定，默认 Software、替代 FemtoVG，每份构建恰好启用一种渲染器。当前目录及命令以本文和[开发说明](development.md)为准；旧验收记录中的 `rust/...` 路径表示当时构建，保留原始校验值。

## 模块

- `src/main.rs`：参数、组件组装与事件循环。
- `src/business.rs`、`src/ui.rs`、`src/file_search_ui.rs`：业务状态、Slint 模型与 Everything 文件模式。
- `src/core/`：配置、串行原子存储、搜索、历史、索引、启动组合、文件查询和更新规则。
- `src/platform/windows/`：窗口、IME、热键、托盘、COM/Shell、Everything Query2 IPC 与生命周期资源。
- `ui/`：页面、组件、主题、动画和资源；UI 线程不扫描、写盘或提取 Shell 图标。
- `tests/`：数据、搜索、发现、页面契约与运行时回归；`tests/fixtures/compat/` 保留脱敏合成基线。
- `examples/`：显式执行的原生测试与测量辅助程序，不是产品常驻进程。
- `windows/`：PE manifest、工具链声明、NSIS 与安装事务脚本；`licenses/` 保存来源与许可。

只有 `renderer-software` 或 `renderer-femtovg` 可以启用，每份构建恰好一个。`product` 决定安装产品身份，预览使用独立身份。程序直接调用原生 API，不加载 Everything SDK DLL；关闭 Everything 集成时不创建查询 worker、消息窗口或结果缓存。

## 调用链与线程边界

`main.rs` 组装窗口、实例锁、生命周期和业务控制器。Slint 回调经界面适配层查询当前 `Arc<Snapshot>`，搜索只访问预归一化的内存索引；显示行与图标模型由界面线程更新。页面、键盘和显隐规则分别沿用 `navigation.rs`、`input.rs`、`lifecycle.rs` 的边界。

`core/service.rs` 通过有界队列协调发现、启动与串行管理、图标和更新任务；各任务调用 `platform/windows/` 完成系统操作。通知合并后回到 Slint 事件循环，索引和文件搜索使用 generation 等机制拒绝过期结果。隐藏窗口会释放文件结果、暂停不必要的界面更新，完全退出时停止任务并释放原生资源。

Workspace 与 Launcher 共用应用进程。布局、动作类型和手势算法在 `core/workspace/`，窗口、侧键钩子和焦点校验在 Windows 平台层，Slint 适配在 `workspace_preview`。模块默认关闭。启用后注册触发键，浮窗在首次呼出时建立并复用。双模式的方向识别由原始触发点和原生输入驱动。自定义动作最多同时执行一个，通过独立的后台线程返回结果。设置事务先预留输入注册，再原子保存和切换绑定。 [Workspace v0.3 实施步骤](plans/velo-workspace-v0.3/implementation.md)

配置与数据先写同目录临时文件、flush，再通过 Windows 原子替换提交；新状态只在成功后发布。有效旧文件读取后不重写，未知字段继续保留，损坏或未来版本数据保留原件并报告问题。来源许可与兼容语义不通过删减代码来规避。

概览见 [README 流程图](diagrams/velo-how-it-works.svg)。新增功能沿用对应层的边界；单个 Windows 应用无需为每层创建独立 crate，也不应把扫描、持久化或句柄所有权放进 Slint 页面。

## 测试与诊断

标准入口见 [开发说明](development.md)。`--diagnostics` 只记录事件类型、状态和数量，不写真实查询或个人路径。`--smoke-test`、`--input-smoke-test` 及业务、页面冒烟是可重复自动检查，不能代替真实键盘、IME、UAC 或跨机器验证。

正常窗口关闭为隐藏，从托盘退出才释放实例、热键、托盘与业务线程。测试副本必须同时指定独立 `--instance-id`、`--data-dir`，必要时独立热键；不能使用正式 `%LOCALAPPDATA%\Velo` 或子目录。`--isolated` 使用固定样例并禁用正式生命周期，适合组件验证。

## 兼容基线与历史

Go 版保存在 [go 分支](https://github.com/HBLADEH/velo-launcher/tree/go)。迁移期差分工具的最后副本还可从整理前提交 `cc65619` 检出。当前 Rust 测试直接比较已固化的 JSON、原始 ID 与搜索静态表；不再动态运行旧语言。删除 Go 源码不改变旧配置只读、未知字段保留、原子保存、升级备份或失败回滚。

`licenses/go-pinyin-MIT.txt`、`licenses/Go-BSD.txt` 是已嵌入拼音和 Unicode 数据的许可，必须保留。它们不表示 Go 编译器或运行时依赖。
