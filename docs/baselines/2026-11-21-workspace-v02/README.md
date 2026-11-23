# Workspace v0.2 验收记录

2026-11-21，北京时间。记录径向工作台在 `codex/workspace-p0` 工作树的本机验证。原版性能对照来自 `db761e8` 的隔离 Release 构建。旧沙漏工作台的截图、结果和构建身份见 [Workspace P0](../2026-11-21-workspace-p0/README.md)。

## 范围

- Windows 11 x64 Insider Preview 26H2，构建 `26340.10013`。初轮采用 1920 × 1080 的单屏 100% 桌面，工作区 1920 × 1032。双屏轮次见下文。
- 运行独立实例和数据目录。桌面脚本通过 Windows 输入队列注入按键与鼠标事件，另设低级钩子见证选中侧键和未绑定侧键的去向。已安装的其他应用仍由用户管理。
- Software 和 FemtoVG 构建分别记录。Windows 11 主机覆盖普通权限桌面、混合 DPI 双屏和实体侧键手势。Windows 10 和 11 客机覆盖双渲染器的高权限前台和真实 UAC 往返。Windows 10 还覆盖四边原生任务栏及 VMware SVGA 3D 上的 FemtoVG 径向专项。

## 已验证行为

两轮 Release 均通过 `./scripts/verify-rust.ps1 -SmokeTest`。Software 和 FemtoVG 各通过 137 项 Rust 测试，各有 1 项显式忽略。用于双屏矩阵与性能采样的构建回执位于本机 `target/workspace-v02-radial/final-verifier-104726/`。

- Software `29D66148B6FFB317CE42A5C7C6FC2662B5AEAC5922E108B0F8729FDE582CE81C`。
- FemtoVG `221011022EEC615704759A948AC527056A2EF6D05BD3606EC667E499F572E5FF`。

设置脚本随后改为等待保存回执才读取 `config.json`，避免与原子替换竞争。源码指纹对照只有 `scripts/test-workspace-preview.ps1` 变化。复验与本地演示使用下列构建，启用、禁用、热键释放、旧侧键透传和槽位持久化均通过。原件见本机 `target/workspace-v02-radial/final-settings-and-display-105646/`。

- Software `D6214F9FA7B1F8E1DAC269FA8185FC7ECFF402FAAB203400A721BABE2C77D32B`。
- FemtoVG `DE88D3E3364F6098996BB4213D40047E8F13D959416C8335EC32954A3CA7450A`。

触发时冻结坐标、显示器、工作区和 DPI。呈现或派发前检测到变化就取消。高完整性应用使用默认布局，动作提交前核对输入桌面。两份更新后的 Release 各通过一次完整 `verify-rust.ps1 -SmokeTest`。FemtoVG 冒烟曾在并行安装客机时出现 OpenGL 有画面、事件计数为零，前一版也复现该结果。复跑通过，原始失败和通过回执均保存在本机 `target/workspace-v02-windows10/`。

- Software `D8E5D91A52E13A831F44F6280304B9F4EE85309D895B7B02D9FA8545A9C4884F`。
- FemtoVG `F4D460EF87124B14F1C7B02DA0CA01CCFFD3582AAE58E886E26114E6C668754A`。
- 两份更新后的构建用合成侧键补验再次短按收起并归还焦点、下划选中测试槽并只派发一次，以及外圈空白点透。两种渲染器都用真实匹配窗口显示应用专属弧片，从设置移除布局并保存，再次呼出采用默认布局，原应用弧片位置恢复点击穿透。本机记录位于 `target/workspace-v02-windows10/host-side-actions-131821/`、`host-additional-132126/` 和 `host-profile-delete-132251/`。

2026-11-22 重新执行 `./scripts/verify-rust.ps1 -SmokeTest`，两种渲染器各有 137 项 Rust 测试通过、1 项忽略，Release 和桌面冒烟均通过。两份构建身份的输入哈希与当前工作树一致。Software 重编译为 `1219EE810A5CB270285AC2F08FC5DE37EBF84BBD4F9EF029AB9DCA75AA0ED643`，FemtoVG 哈希仍为 `F4D460EF87124B14F1C7B02DA0CA01CCFFD3582AAE58E886E26114E6C668754A`。此前客机、实体输入和性能回执沿用各自记录的构建身份。本轮构建回执位于本机 `target/workspace-v02-current-verifier-20261122-123238/`。

同一目录的 `*-radial-desktop.log` 和 `*-radial-retry.log` 保留新版桌面专项脚本的两轮尝试。四次均在启动 Velo 前，因 Windows 的 `SetCursorPos` 返回失败而中止。宿主前台仍是此前停在 EFI 固件错误提示的 VMware 测试客机。这两轮未产生新的径向窗口、跨屏或侧键专项回执。

### 2026-11-22 当前构建复验

- 宿主机 Windows 11 `26340.10013` 恢复桌面输入后，使用上列 12:32 构建运行 `test-workspace-preview.ps1 -Mode radial -CrossMonitors -SettingsRoundTrip`。当时双屏均为 96 DPI，主屏位于 `(0,0)`，副屏位于 `(1920,42)`，两屏分辨率均为 1920 × 1080。各 renderer 以同一个 Workspace HWND 往返，物理尺寸均为 420 × 424。原有 96 / 120 DPI 和负坐标回执继续对应下文的构建。
- Software 用合成 `XBUTTON1`，FemtoVG 用合成 `XBUTTON2`。两份均通过 `Alt+Space`、径向空隙点透、边角夹紧、焦点归还、冷快划单次派发和目标关闭时的零派发。每份 60 次定点绘制采样中空白样本为 0。设置禁用后销毁工作台窗口、释放热键、透传原侧键的按下和释放。启动热键冲突专项中工作台保持隐藏，Launcher 热键可用。各轮均经托盘以退出码 `0` 结束。原始 JSON、日志和截图位于本机 `target/workspace-v02-p4-host-20261122-2158/`。
- 在主屏上为下区配置「启动器」验证动作。两份构建均在下划松开后恰好派发一次，并由 Launcher 取得焦点。侧键来自合成输入。原件见同目录的 `*-lower-release/`。
- 两份在主屏打开 6 + 4 个已配置弧片。在 4 像素间隔、略过轮廓边界的网格中，每份的 4,236 个窗口区域内采样点均命中 Workspace，6,080 个区域外采样点均命中底层见证窗口。常规专项另外以原生鼠标事件验证弧片间隙接到点击。十槽截图和原始记录见同目录的 `*-ten-slots/`。
- 无边框全屏窗口和按完整路径排除的前台程序均通过。见证钩子在两种条件下收到选定侧键的按下和释放，Workspace 保持隐藏，键盘备用热键通过同轮呼出检查。首次启动排除用例因调用命令传入的开关格式有误，在脚本参数解析时中止。修正调用后两种渲染器均正常通过，退出码为 `0`。原件见 `*-fullscreen/`、`*-excluded-retry/` 及对应日志。
- 合成长按达到 8 秒上限时，两个构建都收起、归还焦点并消费配对释放，下一次短按可正常呼出。上区高亮时将副屏由 `(1920,42)` 暂移至 `(1920,74)`，工作台均记录 `display_changed restored=true`，配对释放得到消费，动作派发计数不变。每轮结束后核对桌面排列与工作区恢复，原件见 `*-display-change/`。
- 将副屏改为 `(-1920,0)` 和 125%，主屏保留 100%。两种渲染器均在同一 HWND 上从 120 DPI 副屏的 525 × 530 像素往返 96 DPI 主屏的 420 × 424 像素，边角定位、弧片首帧、空隙点透、焦点归还、托盘退出与热键释放均通过。每份 55 次定点绘制采样中空白样本为 0。前两次分别受到显示设置窗口和桌面指针高亮干扰。关闭设置窗口并在取色前移开指针后复跑通过，原有命中和点击断言仍执行。原件、失败日志和调整后的探针在同目录，最终结果见 `*-mixed-dpi-clear-cursor/`。复验后两屏恢复 100%，副屏回到 `(1920,42)`。

### 初轮单屏专项

09:24 桌面专项用下列 Release 产物。两份均通过 `./scripts/verify-rust.ps1 -SmokeTest` 和 137 项 Rust 测试，每份各有 1 项显式忽略。测量脚本已补充等待异步派发日志和两组对角的夹紧断言。完整构建身份与回执保存在本机 `target/workspace-v02-radial/quick-release-full-verifier-092240/`。

- Software `7733447564FE8984CC60066AB449607DD3E29C928F483166939AB2B2BFD9FE32`。
- FemtoVG `F69C4E220D7B27C83252EC6006DF69EDF3143CE1665AAA64ECC0421DA0AE2500`。

- 两 renderer 的径向、设置禁用、模块关闭和热键冲突各一轮，八轮均从托盘正常退出。冷启动期间完成上划并立即松开时，「启动器」只派发一次。二次呼出、工作区各边夹紧、定点截图与穿透检查通过。每份径向呼出均为 50 次定点读屏，0 次空白或黑色采样。专项日志、截图和结果在本机 `target/workspace-v02-radial/release-desktop-092426/`。
- 键盘打开独立径向窗口。短按侧键时显示完整菜单，长按先只显示 Logo，上下划各显示单侧。松开执行、回中心取消、Esc 取消和未绑定侧键的上下沿透传通过。Software 使用 `XBUTTON1`，FemtoVG 使用 `XBUTTON2`。
- 鼠标点在可见弧片内触发相应动作，点在弧片间隙时底层测试窗收到点击。执行「启动器」后焦点转到 Launcher，隔离 Launcher 的 `Alt+Space` 入口也通过。
- 径向菜单的编辑入口定位到 Launcher 工作台设置。两 renderer 均通过空槽拖入、Enter 交换和 Ctrl+S 保存。启用与禁用的设置落盘，禁用后销毁 Workspace 窗口并释放侧键。配置写入拒绝、原文备份、旧配置和未知字段由 Rust 合约测试覆盖。
- 2026-11-22 在 Windows 11 `26340` 上用两份已通过冒烟的 Release 补验配置往返。通过界面拖入、交换和保存槽位，再点击「立即备份」。备份与已保存 `config.json` 的 SHA-256 各自一致。从原数据目录重启和从备份生成的独立数据目录启动时，均使用配置中的工作台开关、热键和侧键，无预览参数覆盖。先前的空槽可命中，合成侧键短按和键盘热键都能呼出，启动前后配置哈希一致。每个进程均从托盘退出并释放热键。原件位于本机 `target/workspace-v02-windows10/host-persistence-074351/`。该用例首次安排在 Windows 10 客机，客机登录前蓝屏并进入恢复页，未执行用例。

### Windows 11 双屏 100% / 125%

- 主屏 `DISPLAY2` 在 `(0,0)`，1920 × 1080，96 DPI，工作区 `(0,0,1920,1032)`。副屏 `DISPLAY1` 在 `(-1920,0)`，1920 × 1080，120 DPI，工作区 `(-1920,0,1920,1020)`。从 Windows 显示设置配置缩放和排列，按物理像素记录。
- Software `53825652F4F26F74B1816DE6D357ECBD6343C29F4E42E1E682BA00CF38BDFCFF`，FemtoVG `35AA3AE11E30FBCF34186A5E8116F6A223A6AA7CDC59EA91AE0668DD953E4FC0`。两份通过完整 `./scripts/verify-rust.ps1 -SmokeTest`。构建与回执见本机 `target/workspace-v02-radial/closed-target-full-verifier-094953/`。
- 各 renderer 在两屏分别新建隔离进程，共四轮。验证键盘呼出、冷快划、屏幕 DPI 对齐、两个对角夹紧、透明缝点击送达、焦点归还及托盘退出清理。关闭原窗口时，Workspace 仍持有前台，松开侧键后记录 `reason=target_invalid restored=false`，每轮派发计数均保持原值。输入由桌面测试脚本注入。原始记录见本机 `target/workspace-v02-radial/mixed-dpi-095013/`。
- 早期双屏轮次的副屏见证窗被显示设置覆盖，主屏定点读屏受到桌面指针指示层影响。移开这两个干扰后四轮命中和焦点断言通过。125% Software 的首个完整菜单在「计算器」顶部出现浅色横带，键盘冷呼出和冷快划后的首个完整菜单均可复现。两种渲染器的后续呼出采样显示弧片填充一致。原图见 [Software 首次完整菜单](software-radial-125-white-band.png)、[Software 后续上划](software-upper-125.png)、[FemtoVG 完整菜单](femtovg-radial-125.png)。
- 径向窗口移除 `WS_CAPTION` 并保留其余样式。两份矩阵构建的 125% 键盘冷呼出均通过新增弧片顶部像素断言。跨屏时 Winit 曾再次放大已换算的物理尺寸，从 96 DPI 前往 120 DPI 时达到 656 × 663 像素。窗口现在在 DPI 切换后应用目标尺寸，两 renderer 各用同一 HWND 在 525 × 530 和 420 × 424 像素之间往返。原始记录位于本机 `target/workspace-v02-radial/final-release-matrix-104818/`。 [Software 125%](software-radial-125-final.png) · [FemtoVG 125%](femtovg-radial-125-final.png)
- 两份矩阵构建通过 125% 侧键合成输入、空隙点透、原窗口关闭后的零派发、槽位编辑和退出。初次分组时，桌面测试工具留下的指针高亮覆盖了读屏采样点。清除高亮后重跑通过，保留两轮样本。实际底部任务栏和 Shell 临时占用主屏左、上、右边各 46 像素的独立工作区测试均通过。原件见本机 `target/workspace-v02-radial/final-release-retry-105252/`、`shell-edges-femtovg-104418/` 和 `shell-edges-software-105105/`。
- 两份演示构建均通过 `8 s` 长按超时和配对释放，随后再次短按能打开完整菜单。活动手势中实际将副屏的纵向坐标移动 32 像素，工作台以 `display_changed` 收起，原窗口恢复前台，派发计数不变。原件见本机 `target/workspace-v02-radial/hold-lifecycle-111216/`。复验结束后确认两屏均为 100%，副屏回到扩展桌面的原位置 `(1920,42)`。
- 新增触发时的显示器快照后，两份更新构建分别从 `(-1920,0)` 的 120 DPI 副屏起步，往返主屏再检查当前窗口尺寸、弧片绘制与空隙点透。96 DPI 为 420 × 424 像素，120 DPI 为 525 × 530 像素。两份均通过侧键重复取消、下划单次提交和原窗口关闭后的零派发。输入仍来自桌面测试脚本。原件见本机 `target/workspace-v02-windows10/host-mixed-dpi-133655/`。

### 应用和输入准入专项

- FemtoVG 的应用专项使用隔离用户目录启动 VS Code 与 Edge，配合完整可执行路径匹配。默认组为空的测试配置只在真实匹配时出现下区弧片。两个应用分别收到命令面板、新标签快捷键的提交，派发前确认原窗口重新取得前台。
- Explorer 专项以独立文件夹窗口匹配完整的 `explorer.exe` 路径，点击「此电脑」后收到 Shell 成功回执。应用专项的后续复跑在目标窗口取得前台的步骤超时，保留此前候选的结果和此次失败边界。
- FemtoVG 候选 `6B45CADC577B0CF507EAFA9EDE786B5F5FB66ED461721DB04710E13602B33907` 覆盖无边框全屏窗口和按完整可执行路径排除的应用。测试钩子见证两种情况下选定侧键的按下与松开均透传，Workspace 保持隐藏。原件见本机 `target/workspace-v02-radial/admission-083651/`。
- 10:05 使用双屏轮次的 FemtoVG 构建重跑全套应用快捷键。三个 VS Code 动作与三个 Edge 动作均匹配真实程序路径，动作线程在核对前台后收到 Windows 的 `ShortcutSubmitted` 回执。Edge「书签」的首次测试等待发送后仍由原 HWND 保持前台。单独复跑确认原窗口先恢复前台，随后 Edge 的书签面板接管同一进程内的焦点。日志分别在本机 `target/workspace-v02-radial/mixed-apps-100540/` 和 `target/workspace-v02-radial/edge-bookmarks-100823/`。
- 12:15 使用包含权限回退的 Rust 应用识别模块检查真实前台。完整性级别 `8192` 的普通夹具可识别应用身份，`12288` 的任务管理器回退到默认布局，两次采样期间原窗口均保持前台。此专项覆盖应用识别；使用热键从高权限窗口呼出和 UAC 安全桌面继续单独验收。模块探针 SHA-256 为 `B8E899911A5CE13E09CF7201C600542DBC903B0BE644324C89AC194D335A8B86`，结果见本机 `target/workspace-v02-windows10/host-integrity-121515/`。
- 两份更新构建复跑 VS Code 和 Edge 各三个预置动作，全部取得 `ShortcutSubmitted`。Edge 书签以同一进程接管前台作为回执后的验收条件。Explorer 的本次复跑在测试文件夹取得前台前超时，「此电脑」保留早期候选的 Shell 回执。原件见本机 `target/workspace-v02-windows10/host-apps-132603/` 和 `host-explorer-132905/`。
- 2026-11-22 在 Windows 11 `26340` 上用两份更新构建补跑 Explorer。测试匹配新开目录窗口的 `explorer.exe` 完整路径，两次呼出均命中同一应用槽位。按 `Esc` 后，见证脚本确认原窗口重新取得前台。再次呼出并选择「此电脑」时，两份构建各派发一次，均收到 `Ok(Opened)`，通过 Shell 窗口枚举确认「此电脑」位置已打开。原件位于本机 `target/workspace-v02-windows10/host-explorer-current-074940/`。
- 两份更新构建的无边框全屏和精确路径排除测试均通过。见证钩子各收到一对侧键按下与释放，Workspace 保持隐藏。设置页的热键冲突也在两份构建复测通过，旧注册和配置原文保留，释放冲突后原草稿可直接提交。本机原始记录位于 `host-admission-150630/`、`host-excluded-152318/` 和 `host-settings-conflict-134924/`。
- Windows 11 构建 `26340`、Edge `159.0.4402.13`，Software 用 `D8E5D91A52E13A831F44F6280304B9F4EE85309D895B7B02D9FA8545A9C4884F` 补验浏览器历史。按完整可执行路径排除 Edge 时，合成 `XBUTTON1` 让隔离浏览器从测试页 B 返回 A，键盘备用键仍可呼出。保留排除规则并从设置关闭 Workspace 后，合成侧键再次返回 A。两次都无 Workspace 动作派发。原件位于本机 `target/workspace-v02-windows10/edge-back-045320/`。
- 同一环境用 Edge 原生 `F11` 模式复跑。确认窗口覆盖整屏后，合成 `XBUTTON1` 返回 A，合成 `XBUTTON2` 前进至 B。Software 和在场的 FemtoVG 前进侧键实例都未接管全屏输入。退出全屏后，Software 接管 `XBUTTON1` 并呼出完整菜单，按 `Esc` 将焦点归还 Edge，历史仍停留在 B。原件位于本机 `target/workspace-v02-windows10/edge-back-045823/`。
- 在排除列表为空的同一配置上补验禁用。工作台启用时，普通窗口中的合成 `XBUTTON1` 打开完整菜单，浏览器历史停留在 B。键盘备用键打开工作台设置，保存关闭后，该热键释放，`XBUTTON1` 让 Edge 从 B 返回 A，期间无 Workspace 动作派发。原件位于本机 `target/workspace-v02-windows10/edge-back-051358/`。
- 两份更新构建的十槽完整布局额外采样，每份有 4236 个窗口内命中、6080 个底层见证窗命中，其中另核对 693 处空隙背景像素。报告的 0 处越界命中及背景差异限本次采样。首次检查因见证窗的消息循环停顿触发系统 `Ghost` 窗口，复跑期间持续处理见证窗消息。截图使用额外填入两个空槽的隔离布局。 [Software 十槽](software-radial-ten-slots.png) · [FemtoVG 十槽](femtovg-radial-ten-slots.png)

### Windows 11 实体侧键

- Software 更新构建绑定 `XBUTTON1`，用户完成双屏操作并报告通过，两屏均为 100%。应用日志确认短按展开完整菜单，再次短按以 `side_toggle restored=true` 收起。长按出现 Logo，上下区和槽位高亮均有记录。一次下区释放先取消并归还焦点，另两次释放分别派发图片目录和任务管理器。用户报告与实际派发的动作名分别保留。测试副本退出后验证其独立热键已释放。原件位于本机 `target/workspace-v02-windows10/physical-software-170455/`。
- 2026-11-22，用户确认 FemtoVG 的前进侧键 `XBUTTON2` 能打开工作台。应用日志记录两次短按展开完整菜单，再次按键收起并归还焦点。另有一次长按显示 Logo，松开后取消并归还焦点。这组操作的工作台 DPI 为 96，派发计数为零。原件位于本机 `target/workspace-v02-windows10/physical-femtovg-171702/measurement.log`。
- 同日补测，用户确认前进侧键上划松开执行、副屏下划后回 Logo 取消均可用。同一份日志确认上区展开后「控制面板」只派发一次并收到 `Ok(Opened)` 回执，两次长按松开取消均归还焦点。副屏下划后回到 Logo 的操作过程由用户实测确认。
- 23:37 使用上列 12:32 的 FemtoVG 构建与隔离 Edge 测试页补测实体前进键。Windows 11 当时只有一块活动显示器，缩放为 100%。两次长按松开均取消且归还焦点，其中一次识别出下区并高亮「启动器」，松开时已返回 Logo。派发次数为 0。原件位于本机 `target/workspace-v02-p4-physical-20261122-2331/femtovg-gesture-233326/`。
- 23:49 用同一 FemtoVG 构建继续实测。实体前进键下划后，在「启动器」弧片内原地松开。两轮均记录下区第 2 槽提交，各派发一次「启动器」并切到 Launcher。回到空位或 Logo 后松开的两轮均为零派发并归还焦点。原件位于本机 `target/workspace-v02-p4-physical-20261122-2331/femtovg-gesture-234856/`。
- 同一晚在 Windows 11 `26340.10013`、Edge `159.0.4402.13` 验证实体侧键。工作台开启且 Edge 处于 F11 全屏时，浏览器窗口覆盖 1920 × 1080，实体后退、前进均更新浏览历史，期间浏览器保持前台且工作台没有接管。普通窗口下分别测试按完整路径排除 Edge 和禁用工作台，两种状态的实体后退、前进均通过，排除时 `Ctrl+Alt+F10` 仍能呼出，按 `Esc` 归还焦点。三个隔离用例均经托盘以退出码 `0` 结束并释放热键。见同目录的 `femtovg-gesture-234856/`、`femtovg-excluded-235027/` 与 `femtovg-disabled-235158/`。
- 23:54 用户在鼠标驱动中将实体前进键临时映射为 `F8`。隔离 Edge 记录收到一次 `F8`，浏览器保持前台，工作台日志无同步侧键接管或派发。随后 `Ctrl+Alt+F10` 打开工作台，`Esc` 归还焦点。用户确认恢复原映射。原件见同目录的 `femtovg-gesture-235308/`。

### Windows 11 26H2 客机

- VMware 隔离客机运行 Windows 11 企业评估版 `26300`。简体中文 x64 镜像的 SHA-256 为 `B37A06D5121EEE017826C0B133250BBD21BEC694C2D607C60EDD29090E667A84`，与微软公布值一致。初次从命令行启动时，安装检查报告缺少 TPM。通过 Workstation 启动自动添加虚拟 TPM 后，安装在启用安全启动的客机内完成。 [微软评估中心](https://www.microsoft.com/en-us/evalcenter/download-windows-11-enterprise) · [微软校验值](https://support.microsoft.com/en-us/servicing/os/windows/docs/2026/09/verify-the-authenticity-of-a-windows-11-enterprise-evaluation-iso-file)
- Software 与应用本地 Mesa llvmpipe 上的 FemtoVG 均通过真实 UAC 往返。高亮时切入安全桌面，工作台收起。提示自行结束后，合成 `XBUTTON1` 配对释放被消费，两段均为零派发。键盘备用键和新一轮侧键短按可再次呼出。两轮托盘退出码为 `0`，热键均已释放。
- 本轮任务管理器和 Software 的完整性级别均为 `8192`。第二轮 FemtoVG 在检查到已有任务管理器进程时停止。原件与两套 UAC 回执位于本机 `target/workspace-v02-windows11/win11-26300-permissions-first/`。
- 用同一见证程序的普通权限和高权限窗口复跑。Software 以 `8192` 运行，普通窗口可匹配专属槽位，高权限窗口以 `12288` 持有前台时，虚拟键盘热键呼出默认布局，专属槽位可穿透。夹具随后发送 `Esc`，系统打开任务管理器，焦点归还断言未通过。FemtoVG 的普通见证窗口未取得前台，未到达高权限呼出检查。原件位于本机 `target/workspace-v02-windows11/win11-26300-integrity-witness-initial/` 和 `target/workspace-v02-windows11/win11-26300-integrity-witness-retry/`。
- 专项夹具已加入对 `Ctrl`、`Shift` 和 `F10` 的实际释放检查，再发送 `Esc`。新版夹具已写入，客机下一次启动在 EFI 固件错误对话框处停住，复跑未开始。错误日志位于本机 `target/workspace-v02-windows11/witness-keyup-dual-vcpu-boot-failure.log`。
- 通过 SHA-256 核对测试盘的独立副本后，在单虚拟核客机上复跑。Software 的普通权限应用槽位命中，高权限前台呼出默认布局。见证程序确认组合键已释放，按 `Esc` 后原 HWND 重新成为前台，派发数为 0。该次应用日志记录 `reason=escape restored=false`，焦点结果以独立见证程序实际读取的 HWND 为准。FemtoVG 仍在普通权限见证窗口获取前台时超时。回执位于本机 `target/workspace-v02-windows11/win11-26300-integrity-witness-recovery/`。
- 在同一隔离副本上为见证窗口加入主动焦点交接。Software 再次通过。FemtoVG 的普通见证窗口获得前台，且工作台日志记录呼出，随后在夹具读取窗口前收到 `Esc`。该轮保留为夹具时序失败，回执位于本机 `target/workspace-v02-windows11/win11-26300-integrity-witness-focus-handoff/`。
- FemtoVG 使用同一 Release 与 Mesa llvmpipe 单独复跑，两次呼出都走虚拟键盘的注册热键路径。同一程序的普通权限窗口命中已配置槽位，高权限窗口以 `12288` 持有前台时，`8192` 的工作台显示默认布局，应用专属槽位可穿透。组合键释放后按 `Esc`，见证程序读取到原 HWND 恢复前台，派发数为 0。应用日志仍记录收起瞬间的 `restored=false`。回执位于本机 `target/workspace-v02-windows11/win11-26300-integrity-witness-virtual-keyboard/`。

### Windows 10 22H2 客机

- 在 VMware 隔离客机运行 Windows 10 Pro x64，构建 `19045`，1024 × 768 / 100%，工作区 1024 × 728，普通权限会话。微软简体中文版 ISO 整文件 SHA-256 为 `D485D370406CBCB68959718817BD12ED87E537A14C885F84962E07136FC4A049`，与官网发布值一致。测试数据来自独立测试账户。原件位于本机 `target/workspace-v02-windows10/win10-native-suite-153123/`。 [微软下载页](https://www.microsoft.com/en-us/software-download/windows10ISO)
- 更新后的 Software 构建通过窗口冒烟、冷快划单次派发、径向绘制、点透、鼠标点击启动器、夹紧和首轮热键冲突。其径向用例在见证钩子等待未绑定侧键时超时。FemtoVG 在 Microsoft 基本显示适配器上启动时报缺少 OpenGL 2.0。加用独立的 Mesa 26.3.0 llvmpipe 后绘出了原生 OpenGL 画面，通过冷快划单次派发、完整径向点透及热键冲突。组件冒烟未收到重绘事件，径向点击启动器等待超时。 [Software 完整菜单](software-win10-radial.png) · [FemtoVG 完整菜单（Mesa）](femtovg-win10-radial.png)
- 两种渲染器都通过旧轮廓的两区控件、`Esc` 和重复热键收起。两个旧轮廓的外部点击用例在等待底层见证窗收到点击并收起 Workspace 时超时。此轮按用例记录，Windows 10 整体验收待定。
- Software 用同一 exe 复跑五组通过，包括径向手势、槽位编辑、启用和禁用、启动热键冲突、旧双扇形和圆角。复跑调整了合成点击的间隔，并由底层测试窗记录未绑定侧键的边沿。每轮独立实例正常退出。FemtoVG 本轮在准备本地 OpenGL 测试运行库时中止，留有复制失败日志。原件见本机 `target/workspace-v02-windows10/win10-native-retry-160841/`。
- FemtoVG 在新建的 Mesa llvmpipe 隔离目录复跑同样五组，功能断言全部通过。第三次窗口冒烟通过，前两次保留已绘制 OpenGL 画面却未收到 Winit 重绘事件的记录。一次径向编辑用例的托盘退出超时，由脚本清理测试 PID，其余四轮正常退出。原件见本机 `target/workspace-v02-windows10/win10-native-femtovg-164743/`。此结果覆盖该客机的 Mesa 渲染路径，本机 VMware 3D 路径曾出现黑屏。
- UAC 专项的前三轮在持续按住侧键进入待提交状态前超时。显式发送 `XBUTTON1` 的复跑确认，Software 在上区高亮后遇到真实 UAC 提示，记录 `input_desktop_changed`，工作台收起，切换期间派发数为 0。试验停在安全桌面阶段，托盘清理受限，脚本结束了测试 PID。回执与日志位于本机 `target/workspace-v02-windows10/win10-guest-uac-xbutton1-183044/`。
- FemtoVG 用 `F4D460EF87124B14F1C7B02DA0CA01CCFFD3582AAE58E886E26114E6C668754A` 在同一客机切到另一个普通输入桌面。返回时拦住已接纳手势的配对释放，派发数为 0，键盘备用键和新一轮侧键短按均可再次呼出。同轮槽位编辑通过。两次测试的托盘键盘选项操作都只收到菜单关闭事件，脚本清理了各自的测试进程。回执与日志位于本机 `target/workspace-v02-windows10/win10-native-closeout-041259/`。
- 同一 FemtoVG 构建再跑槽位编辑和输入桌面恢复两组。专项脚本按实际菜单项边界点击「退出」，两组各收到一次退出事件、进程退出码为 `0`，且可重新注册工作台热键。桌面恢复组也再次确认配对释放、零派发和新一轮输入。测试入口已采用这套菜单定位方式。原件位于本机 `target/workspace-v02-windows10/win10-tray-menu-042403/`。
- 高权限专项首轮在确认任务管理器前台时超时。复跑按实际前台窗口所属进程确认任务管理器完整性级别为 `12288`，Software 工作台为 `8192`，工作台热键已注册。该轮 VMware 界面按键和测试 `WM_HOTKEY` 都没有产生工作台呼出日志。原件位于本机 `target/workspace-v02-windows10/win10-high-integrity-0432/` 和 `target/workspace-v02-windows10/win10-high-integrity-retry/`。
- 复用 Windows 10 `19045` 隔离客机和 Software 构建 `D8E5D91A52E13A831F44F6280304B9F4EE85309D895B7B02D9FA8545A9C4884F`。由 VMware 本机回环控制台向客机虚拟键盘发送 `Ctrl+Shift+F10`。任务管理器以完整性级别 `12288` 持有前台时，`8192` 的工作台显示默认布局，按任务管理器配置的应用专属快捷动作位置可穿透，派发数为 0。按 `Esc` 收起后，任务管理器的原 HWND 重新取得焦点。同次运行后续的 UAC 夹具在打开提示前遇到指针定位失败。原件位于本机 `target/workspace-v02-windows10/win10-permission-console-061037/`。
- 同一 Software 构建在独立的 `XBUTTON1` 用例中完成真实 UAC 往返。上区高亮时，Windows 启动 UAC 并切到安全桌面，工作台失焦收起，鼠标钩子记录 `input_desktop_changed`。提示约 121 秒后自行结束，提权请求返回取消。返回普通桌面后，已接纳手势的配对释放被消费，切入和返回各阶段派发数均为 0。键盘备用键和新的侧键短按分别再次呼出，托盘退出码为 `0`，工作台热键释放。原件位于本机 `target/workspace-v02-windows10/win10-real-uac-return-062052/`。
- 在隔离客机副本上，DISM 成功导入目录签名有效的 VMware SVGA `9.17.04.0001` 驱动。开启 3D 后，宿主选用 DX12，客机进入 Windows 桌面。首次启动的设备日志显示显卡仍由 Microsoft 基本显示驱动接管。FemtoVG 构建 `F4D460EF87124B14F1C7B02DA0CA01CCFFD3582AAE58E886E26114E6C668754A` 在无应用本地 OpenGL 库的目录运行，冒烟以 `1` 退出并报告需要 OpenGL 2.0，未绘制首帧。原件位于本机 `target/workspace-v02-windows10/win10-native-opengl-dx12/`。
- 在同一隔离副本中重新枚举显示设备后，Windows 10 `19045` 加载 VMware SVGA 3D `9.17.4.1`，宿主仍使用 DX12。应用目录无本地 OpenGL DLL，系统存在 VMware OpenGL ICD。同一 FemtoVG 构建的冒烟以 `0` 退出，记录 `NativeOpenGL`、4 次重绘和 `opengl_frame=true`。1024 × 768 / 100% 的径向专项通过首次快划单次派发、上下区选择、取消零派发、边角定位、空隙点透、槽位保存、托盘退出及热键释放。50 次绘制采样中空白样本为 0。侧键来自合成桌面输入。原件位于本机 `target/workspace-v02-windows10/win10-native-opengl-pnp-055132/`。 [VMware SVGA 3D 完整菜单](femtovg-win10-native-opengl-radial.png)
- 在 VMware 原生 OpenGL 副本复跑权限专项时，SVGA 驱动已加载，客机停在黑屏带指针的登录阶段。权限夹具未启动。原件位于本机 `target/workspace-v02-windows10/win10-femtovg-permissions-native-070605/`。
- 同一 FemtoVG 构建在 Windows 10 `19045` 隔离客机使用应用本地 Mesa llvmpipe 通过两项权限用例。真实 UAC 切入时工作台收起，返回后消费合成 `XBUTTON1` 的配对释放，两个阶段均为零派发。键盘备用键和新的侧键短按可再次呼出，托盘退出码为 `0`，热键已释放。高权限任务管理器持有前台时，虚拟键盘的 `Ctrl+Shift+F10` 呼出默认布局，应用专属快捷动作位置可穿透，派发数为 0，按 `Esc` 后焦点归还原 HWND。构建、驱动哈希与回执位于本机 `target/workspace-v02-windows10/win10-femtovg-permissions-mesa-071454/`。
- 将隔离副本临时设为单虚拟核并冷启动后，同一 FemtoVG 构建使用 VMware SVGA 3D `9.17.4.1` 完成真实 UAC 往返。切入和返回各阶段均为零派发，合成侧键的配对释放被消费，键盘与新一轮侧键输入恢复，托盘退出后热键释放。该轮随后锁屏，高权限前台用例未通过。原件位于本机 `target/workspace-v02-windows10/win10-native-gl-uac-075557/`。
- 同一副本与构建单独冷启动复跑高权限用例。任务管理器以 `12288` 的完整性级别持有前台时，`8192` 的工作台收到虚拟键盘热键并显示默认布局，应用专属槽位可穿透，派发数为 0。按 `Esc` 后见证脚本确认原 HWND 恢复前台。构建、SVGA 驱动加载与焦点证据位于本机 `target/workspace-v02-windows10/win10-native-gl-integrity-081209/`。
- 通过 Windows Shell 查询底、左、上、右四边原生任务栏，分别得到工作区 `0,0,1024,728`、`62,0,962,768`、`0,40,1024,728` 和 `0,0,962,768`，格式为 `x,y,宽,高`。会话内切换任务栏的两轮中，部分用例在启动 Velo 前等待见证窗超时，原件位于本机 `target/workspace-v02-windows10/win10-taskbar-063304/` 和 `target/workspace-v02-windows10/win10-taskbar-windowed-064003/`。
- 底部使用后一轮通过样本，左、上、右各自冷启动并独立登录。Software 和 Mesa FemtoVG 共八轮均通过两个对角的工作区夹紧、托盘退出和热键释放，每轮保留两个同屏截图。测试后客机恢复底部任务栏。六轮独立登录的原件位于本机 `target/workspace-v02-windows10/win10-taskbar-cold-064402/`。 [左侧任务栏同屏截图](femtovg-win10-left-taskbar.png)

## 性能样本

采用双屏矩阵 Release 构建，对照同机 `db761e8`。双屏环境中主屏 96 DPI 样本依次比较原版、模块关闭、启用未打开、可见和用后隐藏。数据位于本机 `target/workspace-v02-radial/final-release-matrix-104818/` 和 `target/workspace-v02-radial/final-release-retry-105252/`。

- Software 的 PrivateBytes 为 26.82、26.62、26.23、26.96、27.25 MiB，线程数为 20、19、18、19、19。FemtoVG 为 62.27、61.31、60.43、86.68、81.84 MiB，线程数为 22、22、21、23、23。两份构建的已知 Launcher、托盘宿主及 Workspace 窗口数均为 2、2、2、3、3。
- 约 750 ms 的同进程资源窗口内，Software CPU 占单逻辑核比例依次为 8.20%、4.12%、0.00%、0.00%、0.00%，FemtoVG 为 8.20%、2.01%、6.32%、0.00%、0.00%。另测约 3 秒的使用后空闲，两份均为 0.00%。短窗结果受系统 CPU 计时粒度与 Launcher 后台活动影响。
- 进程启动到后台就绪的单次读数，原版、模块关闭和启用未打开依次为 Software 1601、1366、3941 ms 和 FemtoVG 3729、3359、1949 ms。副屏 120 DPI 的独立键盘样本，Software 首次为 147 ms，随后五次中位数为 138 ms；FemtoVG 为 859 ms 和 239 ms。启动与首帧数据在不同运行间有波动。
- 两种渲染器各完成 8 次长按热样本。Software 的侧键到屏幕见到 Logo 为 P50 271.49 / P95 358.08 ms，进入到高亮为 51.01 / 283.53 ms，释放到 Launcher 动作派发为 77.34 / 120.45 ms。FemtoVG 依次为 239.28 / 271.29、49.18 / 66.36、45.46 / 60.69 ms。第一项包含预设的 180 ms 长按判定，P95 采用最近秩，这个样本量下对应最大值。
- 测量使用合成输入、桌面屏幕像素及应用事件时间戳。帧读数包含桌面合成与轮询开销。更早的同机重复样本见本机 `target/workspace-v02-radial/benchmark-055844/`。

### 本机复验预算

合并 `benchmark-055844`、`release-desktop-092426` 和 `final-release-retry-105252` 的主屏 96 DPI 径向数据，每种渲染器 24 次长按。P50 取中位数、P95 取最近秩，再各向上取整到 10 ms。第三项取 `workspace_side_release` 的事件时间减去 `input_us` 作为释放时间，以同一手势的 `workspace_action_dispatched` 为终点。脚本 JSON 中的 `ReleaseToLauncherMs` 另记录 Launcher 取得前台的读数。

- Software 的 P50 / P95 预算为 250 / 350、50 / 90、50 / 120 ms。用后隐藏相对模块关闭，PrivateBytes 增量预算为 3 MiB。
- FemtoVG 的 P50 / P95 预算为 240 / 270、60 / 290、40 / 60 ms。用后隐藏相对模块关闭，PrivateBytes 增量预算为 23 MiB。
- PrivateBytes 预算由三轮增量的最大值向上取整。隐藏后约 3 秒的单核 CPU 预算为 1%，覆盖原样本的 0.00%–0.51%。FemtoVG 高亮样本包含 283 和 300 ms 两次尖峰，Software 最大为 284 ms。保留这些原样本，后续在相同条件下复采，对照中位数和尾部变化。

### 2026-11-22 当前构建性能复采

使用上列 12:32 构建和干净的 `db761e8` 对照，在 Windows 11 `26340.10013` 主屏 96 DPI 重跑。原版每种渲染器各测一个进程，当前构建为每种渲染器各三对独立的模块关闭与径向进程，每个径向进程测 8 次合成长按。延迟沿用同一口径，P95 取最近秩。单次冷进程启动读数从进程启动到 Launcher 后台就绪。

- 首轮原版、模块关闭、启用未打开、可见和用后隐藏的 PrivateBytes 依次为 Software 26.54、26.50、26.38、27.11、27.28 MiB，FemtoVG 65.33、61.71、65.41、88.68、85.06 MiB。线程数依次为 Software 16、16、18、19、19 和 FemtoVG 19、19、21、23、23。两种渲染器的已知窗口数均为 2、2、2、3、3。约 750 ms 的单核 CPU 依次为 Software 2.08%、2.08%、6.13%、0.00%、0.00% 和 FemtoVG 0.00%、2.05%、0.00%、0.00%、2.10%。三个进程中由原版、模块关闭及启用未打开测得的后台就绪时间，Software 为 2613、1359、1408 ms，FemtoVG 为 2307、2268、1907 ms。
- 合并每种渲染器的 24 次样本，按「侧键到 Logo 绘制、移动到高亮、松开到派发」顺序记录 P50 / P95。Software 为 251.36 / 279.14、30.03 / 50.33、32.76 / 50.75 ms。FemtoVG 为 241.97 / 266.86、49.56 / 51.19、38.06 / 52.80 ms。呼出 P50 分别高于既定 250 / 240 ms 预算 1.36 / 1.97 ms。FemtoVG 首轮呼出 P95 为 288.62 ms，随后两轮为 259.90 和 266.86 ms。
- 用后隐藏相对模块关闭的三轮 PrivateBytes 增量，Software 为 0.785、1.258、0.762 MiB，FemtoVG 为 23.348、16.387、20.984 MiB。FemtoVG 首轮高于 23 MiB 预算 0.348 MiB。各轮在额外约 3 秒的隐藏空闲采样中 CPU 均为 0.00%。逐轮保留超预算样本，既有预算继续用于对照。本机原件和复算结果见 `target/workspace-v02-p4-host-20261122-2158/` 下的 `*-performance*/`、`performance-summary.json` 和 `performance-repeat-summary.json`。

2026-11-23 更新默认 Software 构建的 P4 整进程内存要求。实际打开 Launcher 和工作台后，收起并稳定空闲时，PrivateBytes 目标为 ≤30 MiB，验收上限为 40 MiB。上列三轮的 Software 用后隐藏读数为 26.91–27.43 MiB，均达到目标。FemtoVG 同轮用后隐藏读数为 81.67–85.06 MiB，作为备选构建的资源记录。

复验入口见 [开发说明](../../development.md#workspace-径向预览) 与 [实施步骤](../../plans/velo-workspace-v0.2/implementation.md)。

## 2026-11-23 径向工作台视觉和动效复验

- Software `13C7C27B7629CEE579477FCEF49E909E0E2EA71F759BE0F61A3094D6F234AE22` 和 FemtoVG `1817ACFD1D2D1DE33A24CFE257336B5368BA8BA0FEBC625BED9A285F9984CC1E` 分别通过 Release 构建和 `verify-rust.ps1 -SmokeTest`，各 137 项 Rust 测试通过、1 项忽略。原始回执位于本机 `target/workspace-polish-20261122170154/`。
- Windows 11 `26340.10013` 双屏均为 96 DPI。两份均捕获到中心 Logo 先出现、扇区后展开的原生画面。Logo 阶段外侧弧片位置可以点透，展开后弧片间隙仍能点透。按 Esc 可中断出场并归还焦点，原展开回调不会重开窗口。两份均通过减少动态效果时直接显示完整菜单、同一 HWND 跨屏、合成侧键上下划、取消和托盘退出检查。
- Software 用后隐藏约 3 秒，PrivateBytes 为 26.93 MiB。定点绘制采样 Software 为 0/60 次空白，FemtoVG 首轮为 1/60、复跑为 0/60。保留两轮原件。截图分别位于 `software-native/`、`software-light-reduced/`、`femtovg-native/` 和 `femtovg-light-final/`。
- 前后图同为 100% 实际尺寸。独立看图发现齿轮容易被误读成 Velo 设置，最终版在右上内弧恢复「控制面板」文字，并降低普通弧片描边的对比。下载、文档、图片和编辑图标来自项目已采用的 Fluent System Icons，新增四个图标遵循现有 MIT 许可。 [Fluent 图标来源](https://github.com/microsoft/fluentui-system-icons/tree/6c2c9075cf8952ef48369b2475233b6e9761efed/assets) · [许可](../../../ui/assets/fluent/LICENSE.txt)

## 2026-11-23 副屏物理拔插

使用上节 Software Release，在 Windows 11 `26340.10032` 上操作实体前进侧键 `XBUTTON2` 和副屏视频线。双屏均为 1920 × 1080、96 DPI。主屏工作区为 `(0,0)–(1920,1032)`，副屏工作区为 `(1920,42)–(3840,1074)`。两轮日志均记录零丢失事件。原始日志、显示器快照与汇总位于本机 `target/workspace-v02-p4-unplug-20261123-1152/`。

- 11:58 副屏断开时，下区已展开，当前没有弧片高亮。应用在收到显示变更事件约 25 ms 后以 `display_changed` 收起，派发计数为零。Windows 随后识别两屏并恢复原工作区。原件见 `software-gesture-115426/`。
- 12:11 复测确认「启动器」弧片已高亮。工作台在按下约 8.17 秒后以 `release_timeout` 收起并归还焦点。Windows 约 5.45 秒后报告副屏断开，这次手势的派发计数为零。原件见 `software-gesture-120521/`。
- 第二轮保持单屏时，在同一进程用合成侧键和 `Ctrl+Alt+F10` 分别重新呼出。原 Workspace HWND 两次均落在主屏工作区内，Esc 收起后均将焦点归还隔离 Edge，恢复检查期间零派发。测试实例在单屏状态下从托盘以退出码 `0` 结束，并释放工作台热键。原件见 `software-gesture-120521/` 下的 `recovery-1-monitor.json`、`exit.json` 和 `final-topology.json`。
- 用户接回副屏后，Windows 恢复原工作区和两屏 96 DPI。用上节的 Software 与 FemtoVG Release 各新启一个隔离实例，在主副屏分别通过合成 `XBUTTON2` 和 `Ctrl+Alt+F10` 呼出。后续三次呼出均复用各实例首次创建的 Workspace HWND，窗口均位于光标所在屏的工作区内。八次 Esc 收起均将焦点归还隔离 Edge，动作派发为零。两份均通过托盘退出并释放 Launcher 和工作台热键。修正首轮探针对尚未创建窗口的零句柄判断后复跑通过。原件见 `after-reconnect-131129/`、`after-reconnect-131530/`、`software-gesture-131531/` 和 `femtovg-gesture-131541/`。

### 同一进程的物理拔插复验

下午继续在 Windows 11 `26340.10032` 上验证上节的 Software Release。两屏仍为 1920 × 1080、96 DPI。通过轮次由操作者在副屏用实体前进侧键上划，并手动拔插副屏视频线。独立探针记录实体侧键的按下与松开、原生窗口和显示器拓扑。

- 15:58 主屏起手和 16:05 副屏起手均先触发 `release_timeout`，分别约 88 ms 和 822 ms 后才收到显示变更事件。两次断开期间，原进程均通过主屏侧键、备用热键和 Esc 焦点归还检查。16:08 的准备试划中，辅助脚本移动了光标但未触发高亮，另一次试划达到长按上限。保留这些原始日志。
- 16:08:30.290，在副屏按下实体 `XBUTTON2`。最后一次高亮为「启动器」弧片，记录于 16:08:32.488。探针确认只剩主屏后，应用在 16:08:34.818 收到显示变更事件，约 75 ms 后以 `display_changed` 收起并归还焦点。实体侧键在 16:08:50.998 松开。本次手势从按下到收起约 4.60 秒。
- 接回副屏后，两屏排列、工作区和 DPI 恢复。保持原进程 `34444` 和同一 Workspace HWND，用桌面探针在主、副屏分别合成侧键和 `Ctrl+Alt+F10` 呼出。四次窗口均在所在屏幕的工作区内，Esc 均将焦点归还隔离 Edge。单屏呼出样本取自同一进程的前两次断开。
- 整段日志共 179 条应用事件，零丢失、零动作派发。隔离实例通过托盘以退出码 `0` 退出，两个测试热键均已释放。原件见本机 `target/workspace-v02-p4-continuous-20261123-155815/software-gesture-155816/` 下的 `continuous-result.json`、`measurement.log`、`physical-xbutton2.ndjson`、`witness-155819.ndjson` 和两份恢复回执。

## 2026-11-24 触发设置与关闭状态复验

- Windows 11 `26340.10032`，单屏 1920 × 1080、96 DPI。Software `A9D6816967CFA4CEBFF722B38ED191FA877305BD7DC98316E269DDD6A360EDC5` 与 FemtoVG `F600A2721902327CAB0FD26B9AEC8C5CEE568370646FF216AD27E4EED45C941C` 均通过完整构建冒烟、关闭状态和径向设置专项。Software 一次径向专项在静置取样前失焦，同一构建复跑通过。原件见本机 `target/workspace-trigger-final-20261124-012544/`。
- 全新配置只启用 Launcher。工作台热键可被见证程序注册，两枚侧键的按下与松开均透传，未建立工作台 HWND、热键线程、鼠标钩子线程或应用识别线程。打开过工作台，再从设置关闭并保存，也通过相同检查。
- 设置页左侧依次选取「不触发」「前进侧键」「后退侧键」并保存，磁盘对应项依次为 `null`、`xbutton2`、`xbutton1`。右侧已设快捷键在三次保存后均按原值读回。两种渲染器各用一枚不同的合成侧键完成径向动作、空隙点透和焦点归还测试。四个隔离实例均从托盘正常退出。本轮范围为当时的单屏桌面。
