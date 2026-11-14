# RS-06：双屏电脑环境配置与补验

执行日期：2026-11-03（北京时间）。分支 `codex/rust-slint-refactor`，HEAD `4d9dbaf9acf5286d9961fae1e89cca908b5af9d7`。本记录补充 [原 RS-06](../2026-10-28-slint-rs06/README.md)，不改标旧构建的结果。

最终进展：[继续记录](followup/README.md)、[实际 S3](sleep-recovery/README.md)和 [FemtoVG 最后拔插](femtovg-unplug/README.md)已补齐全部暂缓实机项，**RS-06 已验证，G2 在已测 Windows 11 x64 范围放行**。逐项证据、构建对应关系和限制见 [G2 验收](g2-acceptance.md)。各轮中途停止和待办保留为当时事实，不代表当前仍缺少这些证据。

**状态：RS-06 已验证，G2 放行（限记录中的 Windows 11 x64 环境）。** 两种渲染器的本机构建、真实混合 DPI 跨屏拖动、负坐标、中文输入、动态尺寸、实际 S3、真实托盘及物理拔插/重新连接全部有通过证据。原显示和电源配置已恢复，测试进程、托盘端点及热键已清理。RS-08 未开始，默认渲染器未决。

## 环境与构建

本机 Windows 11 Pro Insider Preview x64 `10.0.26340`，Intel Core i7-10700、31.8 GiB 内存、Intel UHD Graphics 630（驱动 `30.0.100.9805`），两块活动的 DELL D2721H、各 1920 × 1080。系统还安装有远程显示驱动，完整设备清单见 [environment.json](environment.json)。本轮不是 Windows 10 或全部显卡驱动的兼容验证。

安装 Rust/Cargo `1.99.1-x86_64-pc-windows-msvc`、rustfmt、clippy，并在 `rust/` 设置目录 override；保留原全局 GNU 默认工具链。原用户 Cargo 配置包含无效 TOML 引号和地址，已在本机备份后改用官方 sparse 索引。用户在管理员 PowerShell 执行已校验 SHA-256 与微软签名的 Build Tools 18.0.2 安装脚本，退出码 0、无需重启。原生版本通过现有严格检查：MSVC `14.50.35717`，cl `19.50.35719.0`、link `14.50.35719.0`，SDK `10.0.26100.0`。没有修改工具链锁定文件或 Rust 应用源码。

| 渲染器 | exe 字节数 | SHA-256 | 构建身份 |
| --- | ---: | --- | --- |
| Software | 10,864,128 | `417FF956E78C632E101989586A6CACA30BF0DF7414E13122CC75A26B0338E993` | [software-build.json](software-build.json) |
| FemtoVG | 10,362,880 | `AD4A650FB62AD4A13FA12239C8CC13299748241FD0B26D1B557AEBAE08B7BCB3` | [femtovg-build.json](femtovg-build.json) |

每份身份的 49 个源码、脚本和资源指纹均与实际文件一致；dirty 标记来自说明文档修改。`./scripts/verify-rust.ps1 -SmokeTest` 全部通过：fmt、两种 feature 的 clippy `-D warnings`、各 26 项测试、两份 Release、窗口与输入冒烟，输入冒烟各 20 项断言。FemtoVG 观察到 `NativeOpenGL`；Software 使用独占 feature 与非空白窗口冒烟。见 [checks.json](checks.json)。Rust 应用及共享验证工具未改动，未重跑 Go/Vue 回归；没有重采内存或延迟，也没有替换产品安装包。

## 实测范围

正常模式使用独立实例 `rs06-nov03-femtovg` / `rs06-nov03-software`、各自的 `.reasonix/` 数据隔离路径和 `Ctrl+Alt+K`，没有使用 `--isolated`。FemtoVG 两个进程先后运行，Software 为一个进程；测试实例没有同时运行。原始日志保存在本机忽略目录 `.reasonix/rs06-dual-20261103/`。

初始主屏为 `DISPLAY2 (0,0)`、副屏为 `DISPLAY1 (1920,42)`，均 96 DPI。通过 Windows 显示设置实际改变缩放与排列，未使用 `SLINT_SCALE_FACTOR`。FemtoVG 覆盖主屏 100% / 副屏 150%，随后副屏移到 `(-1920,0)`；Software 覆盖主屏 150% / 左侧副屏 100%。所有跨屏移动均来自 Computer Use 指针拖动，不是 `SetWindowPos` 注入。

| 场景 | 实际结果 | 证据 |
| --- | --- | --- |
| FemtoVG 副屏首次启动 | 从副屏资源管理器实际双击测试快捷方式，首次显示原生焦点已为 true；未点击搜索框即可输入 `code`，窗口为 640 × 264 逻辑像素 | [截图](screenshots/femtovg-secondary-search-100.jpg)，`femtovg-secondary-events.log` |
| 两种渲染器主屏首次启动 | 将指针放在主屏临时夹具区域后启动全新进程；两者在工具输入之前均记录 startup `native_active=true` 与 `native_focus_sync focused=true`，未点击搜索框即可输入 | [FemtoVG](screenshots/femtovg-primary-fresh-input-150.jpg)、[Software](screenshots/software-primary-fresh-input-150.jpg) |
| 混合 DPI 往返拖动 | FemtoVG `A70FA0` / PID 190592 在 144→96→144 DPI 往返；Software `C80D90` / PID 185280 在 144→96→144 DPI 往返。原 HWND、查询和后续输入保留 | [FemtoVG 主屏](screenshots/femtovg-primary-empty-210.jpg)、[Software 副屏](screenshots/software-drag-negative-100.jpg)，原生快照 |
| 负坐标 | 系统显示设置实际将副屏放在 `(-1920,0)`；隐藏窗口保持隐藏，热键呼出仍可访问；两种渲染器均实际从负坐标副屏拖回主屏 | [FemtoVG 呼出](screenshots/femtovg-negative-recalled.jpg)、[拖回主屏](screenshots/femtovg-negative-to-primary.jpg)、[Software](screenshots/software-return-primary-clamped-150.jpg) |
| 真实微软拼音 | 跨屏后通过原生 n、i、Space 按键进行合成与提交；候选框位于输入区域附近，提交“你”，没有误选启动器条目 | [FemtoVG 150% 候选框](screenshots/femtovg-secondary-ime-preedit-150.jpg)、[提交](screenshots/femtovg-secondary-ime-commit-150.jpg)、[Software 100% 候选框](screenshots/software-negative-ime-preedit-100.jpg)、[提交](screenshots/software-negative-ime-commit-100.jpg) |
| 动态高度与工作区 | `code` 两项为 264，空结果为 210，主页/外观/导入为 660；13 项系统快捷请求 720，在 150% 显示器工作区 1008 物理像素下限制为 672 逻辑像素，返回 100% 后恢复 720 | [FemtoVG 受限列表](screenshots/femtovg-secondary-clamped-list-150.jpg)、[滚动到底部](screenshots/femtovg-clamped-list-scrolled.jpg)、[Software 720](screenshots/software-negative-list-720.jpg)、[受限列表](screenshots/software-return-primary-clamped-150.jpg) |
| 页面及草稿保留 | 两种渲染器检查了外观与导入页的 660 高度；Software 未应用的 `Ctrl+Alt+KX` 草稿、当前查询、页面、位置和 HWND 在实际热键显隐后保留，已注册热键仍为 `Ctrl+Alt+K` | [显隐前](screenshots/software-unapplied-draft-before-hide.jpg)、[显隐后](screenshots/software-draft-restored-after-hotkey.jpg)、[导入页](screenshots/software-import-660.jpg) |
| 原生交互返回 | Software 在跨屏后的 150% 主屏打开有真实 owner 的文件选择器，取消后导入页保留，返回搜索无需再点击即可输入；没有选择文件 | [取消后](screenshots/software-import-after-dialog-cancel.jpg)、[实际输入](screenshots/software-focus-after-native-dialog.jpg)，`owned_foreground=true importing=true native_dialog=true action=None` |
| 实际 Shell 托盘 | 用户分别报告两种渲染器的左键、右键菜单、清晰度及退出通过；日志独立确认 FemtoVG 的实际呼出与菜单取消，以及 Software 的托盘“退出”。缺失的菜单动作继续待补证 | [tray-results.json](tray-results.json) |

[desktop-events.json](desktop-events.json)保存三份测量日志及 SHA-256，所有事件 `dropped=0`；[native-observations.json](native-observations.json)保存 19 份原生只读快照与探针身份。本轮没有通过探针注入托盘通知、移动消息或恢复通知。29 张已检查的窗口截图与指纹见 [screenshots.json](screenshots.json)；只发布固定样例界面，不发布显示设置中的个人信息或文件选择器目录内容。

工具截图通常为 640px 宽，不能用图片尺寸代替原生物理尺寸、DPI 或呈现时延。150% 截图中的采集表现与前一轮 Software 150% 截图一致；用户另行报告实际窗口与托盘显示清晰，没有将采集图像当作物理像素清晰度证明。

## 前次会话的清理与待办

停止前用户报告两份程序已退出；最终只读快照没有原型窗口或桌面服务端点，也没有 `velo-slint` 进程。临时 RS-05 前台夹具已清理；独立取得并释放 `Ctrl+Alt+K` 成功。原 Go/Wails 产品初始运行、最终观察未运行，本任务没有对其执行停止或重启命令，不推断退出原因。见 [cleanup.json](cleanup.json)。

前次会话停止时的待办（现已由链接中的后续记录完成）：

1. **先恢复显示配置**：物理 Esc 停止 Computer Use 后没有继续界面输入。目前主屏 `DISPLAY2 (0,0)` 为 150%，副屏 `DISPLAY1 (-1920,0)` 为 100%；原来是两屏 100%，副屏在 `(1920,42)`。恢复后保存原生快照，并关闭本轮打开的显示设置和 `launch` 测试资源管理器窗口。本机已保存仅恢复原位置的脚本 `.reasonix/rs06-dual-20261103/restore-display-position.ps1`，本轮没有执行；缩放仍需通过系统设置恢复。
2. **托盘动作补证**：保留用户“全部通过”的原回复，同时标明本轮日志没有两份程序的 `Settings action=2`，FemtoVG 也没有 `Tray Quit action=3`，Software 没有 `Open action=1` / `MenuClosed action=4`。下一次分别启动测试实例，从实际系统托盘完成缺失菜单操作并核对事件，不用界面齿轮或底栏按钮替代。
3. **Software 副屏全新启动**：本轮已覆盖其主屏全新启动和同一进程中的副屏输入，尚未完成副屏上的另一全新进程启动；准备执行时 Computer Use 被物理 Esc 停止。
4. **物理拔插与真实睡眠/唤醒**：按用户安排留到下次；不得以系统布局变化或只读探针快照代替。

前次会话未在证据缺少时放行；后续补验现已完成，最终 [G2 结论](g2-acceptance.md)取代该中途状态，RS-08 尚未开始。
