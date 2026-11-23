# Velo Workspace P0 实机记录

2026-11-21，北京时间。基线为干净的 `db761e8`，原型来自 `codex/workspace-p0` 工作树。两组均为锁定工具链编译的 x64 预览 Release 构建，每份仅启用 Software 或 FemtoVG。

## 范围

- Windows 11 26H2，构建 `26340.10013`。当前交互会话枚举到一个活动屏幕，1920 × 1080，缩放 100%，工作区 1920 × 1032。跨屏、混合 DPI、实际负坐标和 Windows 10 有待对应环境复验。单元测试覆盖 125% / 150% 换算、负坐标、工作区夹紧及小工作区。
- 本机 `Alt+Space` 被占用，专项使用隔离实例的 Launcher 热键 `Ctrl+Alt+K` 和 Workspace 热键 `Ctrl+Alt+Space`。测试脚本通过 Windows 输入队列发送按键和鼠标事件。后台测试实例使用独立数据目录，桌面上用于验证焦点和点击穿透的窗口属于测试进程。
- 原型使用 Win32 窗口区域同时裁切显示和鼠标命中区。两种模式共用实底 Slint 内容。双扇形采用折线边界，本轮比较显示、命中和焦点，毛玻璃和半透明材质属于后续视觉验证。

## 验收结论

P1 采用双扇形实底和 Win32 窗口区域。Software 和 FemtoVG 在本机均通过绘制、穿透点击和焦点测试。跨屏与混合 DPI 的验收按实际环境继续记录。

- `./scripts/verify-rust.ps1 -SmokeTest` 双渲染器完整通过，每种 127 项 Rust 测试通过，另有 1 项原有忽略测试。首次构建在并行链接时耗尽内存，使用进程范围的 `CARGO_BUILD_JOBS=2` 完整重跑通过。
- 桌面专项共 26 次独立进程运行，包括每种渲染器的基线、关闭预览、双扇形和圆角各 3 次，以及热键冲突各 1 次。两种形状均通过首呼出建窗、上下按钮、穿透点击、焦点归还、Launcher 接管焦点、工作区夹紧和退出释放热键。冲突组收到 Win32 `1409` 后 Launcher 仍可呼出。26 个实例均以退出码 0 从托盘退出。
- 四种渲染器与轮廓组合各采集 150 个显示后屏幕像素样本，均未观察到黑色或空白。截图显示边缘按预期裁切。采样不能覆盖第一帧前或短于轮询间隔的变化。

截图： [Software 双扇形](software-shaped.png) · [Software 圆角](software-rounded.png) · [FemtoVG 双扇形](femtovg-shaped.png) · [FemtoVG 圆角](femtovg-rounded.png)

## 采样口径

- 每次以新进程和新隔离数据目录启动。后台就绪从应用进程内的启动计时到 Launcher 首次进入后台状态，系统文件缓存保持自然状态。快捷键到可见画面从注入按键到指定屏幕采样点绘制，包含 Windows 调度、桌面合成和脚本采样开销。
- 首次窗口显示单独记录，热呼出用后五次的中位数。基线只有 Launcher 读数。工作台轮廓用同一测试脚本和同一采样点比较。
- 隐藏后等待两秒，读取 PrivateBytes，并记录约三秒内的进程 CPU 时间增量。测量进程包含正常 Launcher 业务任务。屏幕像素用于核对显示及点击边缘，帧采样的时间分辨率以脚本轮询为限。

## 同机对照

以下均为中位数。每组依次列出后台就绪、Launcher 首次和热呼出、Workspace 首次和热呼出、隐藏后 PrivateBytes、CPU 占单逻辑核比例。延迟单位为 ms，PrivateBytes 单位为 MiB。

- **Software 基线**　1665 / 549 / 108，25.83 MiB，0.00%。
- **Software P0 关闭**　1467 / 511 / 102，25.08 MiB，0.00%。
- **Software 双扇形**　1666 / 611 / 99，Workspace 67 / 55，26.65 MiB，0.00%。
- **Software 圆角**　1726 / 701 / 120，Workspace 74 / 56，26.82 MiB，1.02%。
- **FemtoVG 基线**　2829 / 597 / 97，63.69 MiB，0.00%。
- **FemtoVG P0 关闭**　2351 / 663 / 120，60.43 MiB，1.01%。
- **FemtoVG 双扇形**　2235 / 761 / 106，Workspace 513 / 46，87.11 MiB，0.50%。
- **FemtoVG 圆角**　1825 / 808 / 109，Workspace 513 / 45，86.75 MiB，0.00%。

与同一产物关闭预览时相比，双扇形用过后隐藏，Software 的 PrivateBytes 中位数增加 1.57 MiB，FemtoVG 增加 26.68 MiB。FemtoVG 的首次显示在本轮约为 0.51 秒，热呼出在 46 ms 左右。后台就绪样本有明显波动，Software 双扇形为 1416–1909 ms，FemtoVG 双扇形为 1859–3725 ms。三次短采样仅描述本机表现，尚未据此设定性能预算。

## 构建身份

- 基线 Software SHA-256 `2C4681041F32CB3799AEE1F99F8B4116A55F764DBBF74674B52D8231B12CED3D`
- 基线 FemtoVG SHA-256 `4C024DA28E22586133A3EE31AB99F61F34512766B51789DDC78691EC253AC2BE`
- P0 Software SHA-256 `3168D09F5AF10307049396ACF7872E85E996B25642CC8C26FCB5C16496C387A0`
- P0 FemtoVG SHA-256 `E011961BD4EEC5357A45DE1861792F2B3AD910DF1C19D232FC4F55D2F2658B21`

本地原件位于 `target/workspace-p0-baseline/`、`target/artifacts/`、`target/workspace-p0-verify-retry.log` 和 `target/workspace-p0-desktop-013150/`。首次同 PowerShell 进程批跑的测试窗前台超时记录保留在 `target/workspace-p0-desktop-012714/`，对照结果取独立进程的重跑样本。

复验入口见 [开发说明](../../development.md#workspace-p0-预览)。
