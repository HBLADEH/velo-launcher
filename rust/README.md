# Velo Rust + Slint

**当前产品主线：Rust 0.1.0，精确标签 `0.1.0`。** Go/Wails 0.9.x 停止维护，现存 Go 代码仅用于历史契约对照。正式开发与发布命令见[开发说明](../docs/development.md)和[版本说明](../docs/releases/0.1.0.md)。下文带日期的原型、内部许可和待验状态保留原构建范围，不代表当前发布策略。

使用 `./scripts/verify-rust.ps1 -Product -SmokeTest` 检查正式身份，在干净提交上通过后由 `package-rust-release.ps1` 生成默认 Software 和替代 FemtoVG 系统安装器。正式普通启动通过系统安装回执使用 `%LOCALAPPDATA%\Velo`，首次打开前建立原文快照；开发预览继续隔离数据。发布不包含独立免安装版，Go/Node/WebView2 不是运行时依赖。

2026-11-15：[G4 在当前机器范围放行](../docs/baselines/2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md)。正式 `product` 身份/系统接管安装器、两 renderer 完整检查、实际接管/故障恢复/升级/Go回退/卸载、普通权限会话、当前 exe 的搜狗真实键盘和清理通过。按用户要求仅考虑现有宿主机及 Windows10 测试客户机，其他不可用实体驱动等记跳过；原性能差异/失败和构建身份保留。此前“未实现/待人工”属于原检查点，未发布或替换宿主机正式安装。

正式候选检查使用 `./scripts/verify-rust.ps1 -Product -SmokeTest`，产物位于 `rust/target/product-artifacts/<renderer>`；普通预览检查和原路径继续使用不带 `-Product` 的入口。两种构建均要求唯一 renderer feature。正式 exe 普通启动需要系统安装回执和 HKLM64 安装路径匹配；显式测试身份与组件冒烟继续使用隔离数据。

系统候选只生成、不执行：`./scripts/package-rust-product.ps1 -Renderer software -OutputDirectory rust/target/product-software -Makensis <固定编译器路径>`。初次接管仅支持核对过 SHA256 的官方 Go v0.9.11 系统安装；沿用程序路径/注册/快捷方式，保留 `.velo-rollback`。退出后运行安装目录的 `uninstall.exe /ROLLBACK` 恢复原 Go 程序、注册和快捷方式；普通卸载保留用户数据、未知根目录文件和恢复档案。每用户在数据层打开前必须完成 `VeloRustMigrationBackups/<安装事务ID>` 原文快照校验；失败时不打开数据层。候选 `0.1.0` 及内部许可/发布边界不因正式身份功能而改变，不能用于替换宿主机正式安装作测试。

2026-11-13 后续：[真实 GitHub CI / Windows 10 续验](../docs/baselines/2026-11-07-slint-rs14/host-completion-2026-11-13/README.md)完成两 renderer 的远端任务及 Windows10虚拟机十组检查，官方Go系统安装/独立Rust数据副本/Go回退和清理也已补齐。一次性runner已清理，测试虚拟机正常关机；其他实体驱动/IME未测，自动原位接管的正式Rust系统安装器尚未实现，G4仍待。下文此前的CI/环境待办保留原日期范围，以此新入口为准。

2026-11-13：[RS-14 续验](../docs/baselines/2026-11-07-slint-rs14/completion-2026-11-13/README.md)选择 **Software 为已测 Windows 11 x64 的默认 renderer**，FemtoVG 保留显式替代构建。[release-policy.json](windows/release-policy.json)供打包记录默认选择；Cargo 仍要求每份构建显式选择唯一 renderer，标准验证仍覆盖两条路径。许可选项/嵌套声明、真实每用户注册与隔离受保护目录的实际 NSIS UAC 确认/取消已补齐。远端 CI、其他兼容环境及正式全系统迁移仍待，G4未放行；下文旧日期“默认未决”保留其原检查点范围。

这是 [Rust + Slint 重构计划](../docs/rust-slint-refactor-plan.md)的独立 Windows x64 工程。RS-01 至 RS-07 已完成记录中的 Windows 11 x64 验证，2026-11-03 [G2 已放行](../docs/baselines/2026-11-03-slint-rs06/g2-acceptance.md)。[RS-08 数据](../docs/baselines/2026-11-03-slint-rs08/README.md)、[RS-09 搜索/历史](../docs/baselines/2026-11-03-slint-rs09/README.md)与 [RS-10 发现/图标/启动](../docs/baselines/2026-11-03-slint-rs10/README.md)已接通；正常模式读取独立原型数据并后台发现应用。RS-11 已完成本机完整设置、搜索网格、右键动作、自定义应用、备份、登录启动和更新链路验收，验收范围见 [RS-11 记录](../docs/baselines/2026-11-03-slint-rs11/README.md)。最新状态以 [2026-11-04 补验与 G3 结论](../docs/baselines/2026-11-04-slint-rs11/g3-acceptance.md)为准，默认渲染器未决。版本 `0.1.0` 表示验证程序版本，现有 Velo 产品版本仍为 `0.9.11`。

RS-04 / G1 的固定原型可见 PrivateBytes 中位数为 Software 6.82 MiB / FemtoVG 137.21 MiB，只有 Software 达到 30 MiB 初始预算；这是 RS-04 构建的数据，最终渲染器未决。首轮 A/B、构建身份及测量限制见 [RS-04](../docs/baselines/2026-10-28-slint-rs04/README.md)。RS-03 的本机微软拼音与真实系统 100%/125%/150%/200% DPI 证据继续有效，本轮桌面验收没有重测性能或扩大兼容承诺。

另一台双屏电脑可按 [跨机器复验清单](../docs/rust-slint-dual-monitor-testing.md)拉取分支、构建两种渲染器，并记录 RS-06 的多屏/混合 DPI、拔插/恢复、实际托盘入口和清理。Git 不包含本地 exe，需要先安装下表锁定的工具链。

2026-11-03 在双屏电脑完成两种渲染器的完整构建、真实跨屏拖动、100%/150% 混合 DPI、负坐标、中文输入、真实托盘、两种实际 S3，以及 [Software](../docs/baselines/2026-11-03-slint-rs06/followup/README.md)和 [FemtoVG](../docs/baselines/2026-11-03-slint-rs06/femtovg-unplug/README.md)的物理拔插/重新连接。临时电源值与原显示配置已恢复，实例、托盘和热键已清理；完整 [G2 证据与适用范围](../docs/baselines/2026-11-03-slint-rs06/g2-acceptance.md)保留各轮构建身份。

正常模式为无边框置顶窗口，主页和外观设置使用 640 × 660 逻辑像素，列表高度沿用 Vue 的 `156 + max(1, 结果数) × 54`，限制在 160–720；整个窗口再约束到当前显示器的物理工作区。空结果为 210 高，内容放不下时滚动内层区域；外观面板也能滚动。顶部空白区域可拖动，输入框的选择/编辑和按钮保留原操作，拖动位置在当前会话的显隐及尺寸变化后保留。隔离与旧测量模式继续使用固定 28 项数据和 640 × 540 窗口，避免改写 RS-04 的采样条件。页面包括搜索框、六列分组首页、搜索列表、深浅主题、选中/悬停/空结果和减少动画开关。

RS-09 的归一化、拼音/首字母/别名、关键词、模糊匹配、历史加权和稳定排序现在查询后台发布的真实快照。正常模式点击或 Enter 发出 Shell 启动请求，立即清空查询；`config.json` 的 `space_launch` 决定空格是否启动。成功后学习并原子保存历史，失败显示可操作提示且不学习。首页四方向键遵循 Vue 的跨分组、不完整行和循环规则，列表上下循环、左右保留光标编辑，选中项保持可见。IME 合成期间 Enter/Space/方向键/Esc 仍交给输入法。

`--fixtures` 保留原固定样例；`--isolated`、两种冒烟和未加 `--measurement-business` 的 `--measurement-stdio` 自动使用样例。固定时钟与合成历史保持 3 个固定项、12 个常用项和 13 个系统快捷的顺序，点击/Enter 只记录选择，Space 是普通文本。该模式不读取业务文件或执行样例路径。

顶栏齿轮在正常模式打开七个设置页，固定样例保留外观与关于面板；Esc 优先关闭面板，其次清空查询，空查询时隐藏。Alt+Space 显隐；关闭窗口（包括 Alt+F4）隐藏，托盘菜单及底栏“退出”结束进程并释放热键、托盘和实例锁。托盘左键呼出，右键有打开/设置/退出；菜单临时获得焦点不会隐藏主窗口，取消后按实际前台归属恢复焦点或隐藏。失焦到无关窗口时隐藏，所属弹窗不触发这一转换。每次主页呼出恢复原生及搜索输入焦点，显隐保留查询、面板、主题和快捷键草稿。RS-07 已完成本机普通权限的原生文件选择器、右键菜单与确认框，以及普通 Explorer 到管理员目标的阻止边界、管理员同权限 OLE、选择文件回退和返回输入；其他实机边界见 RS-06 与 RS-07 的适用范围。

`--background` 等原生窗口、固定模型及热键注册就绪后保持隐藏；热键注册失败则显示窗口和提示，托盘可用时仍可关闭隐藏并再次呼出；热键和托盘都不可用时不允许隐藏，关闭可以退出。面板支持输入或录制快捷键、应用/重绑及全屏禁用开关；录制期间按当前已注册热键只填入草稿，不会隐藏。先取得新注册再释放旧注册，冲突时旧热键仍有效。正常模式保存设置会预注册新热键并保留旧注册，提交登录启动项与配置后才切换；失败释放预注册并恢复启动项。显隐保留未保存草稿，取消不写盘。

正常模式在 Slint 初始化前取得 Windows 会话内的命名实例锁和数据目录锁，再由后台读取配置、历史、应用库和索引；再次启动通过原生消息唤起正确实例。默认实例为 `prototype`，数据目录为 `%LOCALAPPDATA%\VeloSlintPrototype`，首次运行会创建配置并保存索引，成功启动后保存历史。测试副本必须同时指定 `--instance-id=ID --data-dir=PATH`（ID 为 1–48 个英文字母/数字/`-`/`_`）；不同 ID 不能共享同一目录，禁止指向 Go 的 `%LOCALAPPDATA%\Velo` 及其子目录，已有路径的 junction/symlink 会解析后检查。正式产品目录切换留待后续集成和发布验收。

底栏“应用管理”支持多选文件、拖入或粘贴完整路径，检查后点击导入，将 .exe / .lnk 加入持久化应用库；每批最多 100 个，自定义应用最多 1000 个。导入解析快捷方式参数与工作目录，不执行或自动固定。移除自定义应用及其固定项使用一次原子提交；扫描范围内的条目仍由索引管理。固定样例继续使用会话预览。导入页失焦保持显示，Esc 返回原查询；顶部空白处右键或“更多…”打开所属原生菜单，清空先弹出默认取消的确认框。真实右键菜单支持打开、打开安装目录、管理员启动和固定/取消固定；不支持的动作按文件能力禁用。Go 版若已注册 Alt+Space，可用 `--hotkey=Ctrl+Alt+K` 独立验证。`--isolated` 无托盘、实例锁、全局热键或失焦隐藏，关闭直接退出；不能与 `--background` 混用。

## RS-08 数据层与离线兼容验证

`src/lib.rs` 导出 `core/`，供原生业务与离线验证复用。`model.rs` 定义完整 `AppItem`、与 Go 一致的 ID 和旧 `/icons/<hash>.png` 本地路径映射；`config.rs` 固化 version=1 的默认值/范围；`data.rs` 管理历史/索引/应用库格式、一次迁移与加载结果；`storage.rs` 串行提交并发布不可变 `Arc` 快照，磁盘操作不持有读锁。Windows 文件提交集中在 `platform/windows/atomic_file.rs`。

所有 I/O API 均为同步后台接口，调用方先取得实例/数据目录锁，再在后台调用。RS-10 的正常模式已通过 `core/service.rs` 接线；Slint 回调只查询内存和投递有界命令。固定样例/冒烟不启动业务线程。RS-11 的设置、备份和应用库事务复用该串行提交边界。

- 有效旧配置只在内存补缺省字段；保存保留根级和嵌套未知字段。损坏配置先生成不可覆盖的原文副本，再恢复默认值；恢复写入失败时原路径也保留。未来版本配置拒绝加载，保留原件。
- 历史保留 ID 键、计数、查询映射和 RFC3339 原时区/纳秒字符串；损坏历史、损坏/未来应用库返回错误，不以空库替换。索引不兼容时保留原文件并返回待重建状态，重扫归 RS-10。
- 自定义应用与固定项使用同一 JSON 原子提交。已有 `application-library.json` 时始终以它为准；否则仅迁移一次 `quick-launch.json`，保留旧文件及全部原 ID。
- 写入使用同目录临时文件、`sync_all`、关闭文件和 `MoveFileExW`（`WRITE_THROUGH`；数据替换允许覆盖，备份禁止覆盖）。备份在同一提交锁内读取磁盘原文，保留格式、未知字段与先前备份。

共享输入、固定时钟、Go 生成的预期及维护方式见 [兼容用例](../tests/fixtures/compat/README.md)。标准 Rust 验证入口为每个渲染器运行两种语言的完整往返和 NTFS 拒绝写入检查，因此 RS-08 起完整检查也需要 Go；`-SkipChecks` 仍只用于迭代构建。

```powershell
# 单独复验离线契约；全程只使用自动创建的合成临时目录：
./scripts/test-data-compat.ps1 -Renderer Software
./scripts/test-data-compat.ps1 -Renderer FemtoVG
# 标准入口包含格式、clippy、单测、两种数据往返与两种 Release：
./scripts/verify-rust.ps1
```

直接新增的依赖均锁定版本：`serde 1.0.229`（derive）和 `serde_json 1.0.154`（arbitrary_precision）用于 JSON 与未知数值保留；`chrono 0.4.45` 仅启用 std，用于 RFC3339 校验与 UTC 备份时间命名；`tempfile 3.27.0` 用于独占临时文件及失败清理；这些版本原已存在于 lockfile 的构建依赖图。`sha2 0.10.9` 禁用默认 features，实现兼容 ID，新增 7 个锁定包（含自身），无原生加密库依赖。上述依赖为 MIT / Apache-2.0 许可；Windows 文件 API 只增加已有 `windows-sys` 的 `Win32_Storage_FileSystem` feature。资源开销需后续同轮测量，本阶段没有新增内存收益结论。

## RS-09 搜索、历史与首页

`core/normalize.rs` 固定 Go 的 Unicode 简单小写和分词规则；`search.rs` 在构建快照时生成拼音后缀与别名，查询按匹配等级、来源/固定/历史权重、名称和 ID 取前 N 项。非空查询的加权限制在 ±180，完整名称额外加分，精确结果不被固定/高频项挤掉。`catalog.rs` 合并已过滤应用、自定义项、固定项和调用方提供的系统工具，保持应用/工具的稳定合并、首页固定顺序、排除重复固定项、12 项常用列表及工具的使用频率顺序。发现、过滤和 RS-11 的系统工具目录均已接通。

`history.rs` 实现次数、原始精度时间、query→app 和每应用 100 条查询记录；`Store::record_success` 仅在启动成功后调用，`Store::set_pinned` 在串行事务中变更应用库，持久化成功后才发布新快照。失败保留旧状态，读者无需等磁盘写入。超过 i64 的学习计数拒绝写入；Go 未定义同次数查询的淘汰顺序，Rust 在同最小次数下按查询键排序，保留当前查询。

拼音和 Unicode 表由现有 Go 基线生成，直接嵌入约 346 KiB 只读数据，无新 Cargo 依赖或常驻 Go 进程。来源、表结构及许可证见 [search_data](src/core/search_data/README.md)。这不是新增内存测量，RS-04 的预算结论不转用于当前构建。

```powershell
./scripts/test-search-compat.ps1 -Renderer Software
./scripts/test-search-compat.ps1 -Renderer FemtoVG
# 标准入口也验证 Go oracle、静态表、共享 fixture 及两种 Rust 构建：
./scripts/verify-rust.ps1 -SmokeTest
```

共享用例见 [RS-09 fixture](../tests/fixtures/compat/rs09.json)和[维护说明](../tests/fixtures/compat/README.md)。Go 原服务仅提取私有固定时钟入口，生产入口仍使用当前时钟；差分直接调用既有搜索、历史与首页实现。最终验证记录见 [RS-09 证据](../docs/baselines/2026-11-03-slint-rs09/README.md)。

## RS-10 发现、图标与启动

`core/indexer.rs` 保留 Go 的扫描深度、桌面快捷方式有效性、来源优先级、别名合并、重复名/目标和辅助项过滤。仅重新解析时间/大小变化的文件；不可访问子树保留旧条目，已删除文件移出快照，瞬时快捷方式错误下次重试。Windows Known Folders、`IShellLinkW`、原生 AppsFolder、Shell 图像和启动集中在 `platform/windows/`；发现、图标和启动不调用 PowerShell、Go、HTTP 服务或 WebView。

RS-10 的三个有界业务线程负责发现/刷新、Shell 动作及历史写入、可见图标；RS-11 增加一个有界更新线程。刷新队列容量 1，重复请求取消旧 generation 并合并；启动同时只接受一个请求；图标邮箱只保留最新可见批次，最多 96 项；UI 通知按类型合并。窗口先显示已有内存/缓存内容，按配置周期或“刷新应用”后台更新。索引成功持久化后才发布；取消、过期和写失败不发布或清理图标。不兼容/损坏索引先保留原文副本，再重建。

旧 `/icons/<hash>.png` 直接映射到本地 PNG，签名与 Go 一致。图标按可见范围解码/提取，隐藏或切换页面使请求过期，离屏模型释放图片引用。应用持有的解码图片与纹理估算采用 8 MiB 预算：4 MiB LRU，4 MiB 留给传输、解码和 Shell 临时位图；淘汰保护仍可见图片，缺失图标显示文字。64 px 原图沿用 Go 基线，Windows 100–200% DPI 的 32 px 图标可复用。渲染器内部纹理缓存及总进程内存必须在 RS-12 另测。

普通启动执行 `.lnk` 本身，保留 Shell 的参数、工作目录及提权标志；打包应用交给系统 Explorer。后台动作层及 RS-11 右键菜单支持管理员启动与打开安装目录；管理员操作保留解析后的参数/工作目录，UAC 取消作为失败，不记历史。正常模式隐藏后启动，失败仅在生命周期 generation 仍匹配时恢复；查询不会回滚。所有业务线程在退出时取消、唤醒并回收，原生同步 Shell 调用在返回后观察取消。

```powershell
./scripts/test-discovery-compat.ps1 -Renderer Software
./scripts/test-discovery-compat.ps1 -Renderer FemtoVG
# 在 rust/ 且已初始化锁定的 MSVC 环境中；只操作自动创建的合成目录：
cargo run --locked --no-default-features --features renderer-software --example rs10_windows -- target/rs10-native-new.json
```

原生验证对照本机 Go 快捷方式、Known Folders、AppsFolder 及图标签名，只启动自己的合成 helper；不执行已安装应用或修改系统设置。报告不得覆盖已有文件。右键菜单与完整导入已在 RS-11 接通，真实 UAC 桌面与系统动作的实机边界见 RS-11 记录。

直接增加 `windows 0.62.2` 的 Foundation/GDI/FileSystem/COM/Shell/Common features 和 `image 0.25.10` 的 PNG feature（均关闭 default features）；二者已在原 lockfile 依赖图中，许可证 MIT / Apache-2.0。类型化 COM 接口提供 RAII，PNG 使用既有编解码依赖；没有增加通用图像格式。`windows-sys` 的 Registry feature 仅为 `SHELLEXECUTEINFOW` 类型依赖，本阶段不修改登录启动注册表。资源成本以后续测量为准。

## RS-11 页面、设置与系统集成

`ui/settings.slint` 管理常规、快捷键、外观、搜索、索引、备份和关于七个页签。`business.rs` 适配草稿和回调，`core/service.rs` 的有界命令队列执行保存、固定、导入、移除与备份；UI 不扫描磁盘或写文件。正常搜索可使用六列网格及四方向键，图标仍只按可见范围加载。

`core/library.rs` 保留导入 ID、参数与工作目录、去重与上限；`core/system_tools.json` 从旧 Go 目录提取，Go 回归防止目录定义漂移。本机根据 Windows build、文件存在性、协议注册及用户/机器策略筛选工具。协议和电源动作只接受精确的内置身份；关机/重启默认取消，清空回收站使用 Velo 所属原生确认，保留 Windows 清理进度与声音，取消不调用清理或学习历史。

`platform/windows/preferences.rs` 管理系统主题、减少动画和登录启动。原型使用 `VeloSlint-<instance>` 注册表值，并携带独立实例和数据目录，避免覆盖正式 `Velo` 登录项。启用时检查 Windows Run 的 260 个 UTF-16 单元命令限制，超长时提示缩短程序或数据目录，保留原配置和热键；关闭已有超长值仍可成功。保存已启用设置会刷新命令，涵盖程序移动后的旧值。设置写失败恢复注册表原类型/原字节，旧热键保留到提交成功。

`core/update.rs` 与 `platform/windows/http.rs` 使用 WinHTTP 检查 GitHub API，限流时读取公开 Atom/资产页；兼容环境 HTTP/HTTPS 代理与 Windows 系统代理。自动检查延迟 12 秒，只提示，不下载。网络读写、进度和退出取消在独立有界线程运行。下载先写临时文件，校验 SHA-256、flush 后原子提交；备份成功后才能写更新脚本。一次性 PowerShell 脚本再次校验、等待旧进程退出、替换或提权安装并携带原型隔离参数重启。

当前原型只接受 `velo-slint.exe` / `velo-slint-x64-installer.exe` 及 `SHA256SUMS.txt`，产品发布切换时再统一命名；已有 Go 发布不会自动替换 Rust 原型。缺少匹配包或校验记录时明确阻止安装。安装器/UAC、登录后实际启动等实机边界见 [RS-11 验收](../docs/baselines/2026-11-03-slint-rs11/README.md)及 [2026-11-04 人工补验](../docs/baselines/2026-11-04-slint-rs11/README.md)，不以脚本生成测试代替。

新增直接依赖 `quick-xml = 0.41.0`，仅启用 `serialize`（用于受控 Atom 解析），MIT 许可；复用已有锁定版本和 serde，未引入网络运行时。Windows API 仅扩展已有 `windows-sys` features。资源影响待 RS-12 同轮测量。

```powershell
./scripts/verify-rust.ps1 -SmokeTest
# 标准入口增加 RS-11 Go oracle、原生更新替换/恢复、七页截图及重启检查。
# 仅在测试副本中验证真实旧数据（原件只读）：
./scripts/test-pages-ui.ps1 -Executable rust/target/artifacts/software/velo-slint.exe `
  -Helper rust/target/x86_64-pc-windows-msvc/debug/examples/rs10_windows.exe `
  -OutputDirectory rust/target/rs11-old-copy -LegacyCopySource "$env:LOCALAPPDATA/Velo"
```

`--pages-smoke-test` / `--pages-restart-test` 必须显式指定独立实例、数据目录和合成查询；它们检查组件回调及磁盘读回，不代表真实 IME。测试会保存浅/深七页、网格、应用管理和备份截图。所有原始日志、旧数据副本与构建产物只留在忽略的 `target/`。

2026-11-04 的回收站取消及 Windows PowerShell 5.1 路径修复、79 项 Rust 测试与人工取消复验保留在 [该轮记录](../docs/baselines/2026-11-04-slint-rs11/README.md)。实际登录补验又发现 Run 命令超出 260 字符限制；当前长度保护构建的两种路径各 80 项测试、完整组件/页面、三个原生启动场景及旧数据副本通过。两种构建的真实登录自动启动、后台隐藏、热键和中文输入也已通过，临时资源已清理，前两次失败及注册表视图差异保留，见 [最新登录验收](../docs/baselines/2026-11-04-slint-rs11/login/README.md)；随后两种构建的 [真实 UAC 确认/取消与更新恢复](../docs/baselines/2026-11-04-slint-rs11/uac/README.md)也通过，测试实例均正常退出并清理；[G3 在本机 Windows 11 x64 范围放行](../docs/baselines/2026-11-04-slint-rs11/g3-acceptance.md)，RS-11 完成。

可选的 `scripts/test-installer-update.ps1` 在 `rust/target` 内使用本地 installer fixture，验证普通权限不可写目标的提权替换、安装失败保留/重启原程序、原文备份和临时权限/托盘/进程清理；用另一种渲染器作为旧目标，确保替换前后字节确实不同。它只在当前 UAC 策略允许无提示提权时执行，不改安全策略。两种构建共四个成功/失败场景已通过；该入口与真实 NSIS 发布验收分开记录。调用参数见补验记录；helper 可在已初始化锁定 MSVC 的 `rust/` 中用 `cargo build --locked --no-default-features --features renderer-software --example rs11_installer_fixture` 构建。

进一步的 [后台启动补验](../docs/baselines/2026-11-04-slint-rs11/startup/README.md)覆盖当前会话的后台启动参数。首次显隐等配置和实际快捷键注册就绪，避免默认 Alt+Space 冲突时错误弹窗；读取失败仍显示窗口。完整验证入口每种 Release 执行默认键冲突、保存键冲突、未来配置保护三个真实进程场景，不创建登录项或模拟用户登录。当前两种 Release 的完整检查、真实旧数据副本与更新后重启复验通过；用户辅助的 [实际登录验收](../docs/baselines/2026-11-04-slint-rs11/login/README.md)使用另外准备的独立实例和临时登录项。


## 已锁定的构建环境

| 项目 | 版本/选择 | 锁定位置 |
| --- | --- | --- |
| Rust / Cargo | 1.99.1，MSVC host，edition 2024 | `rust-toolchain.toml`、`Cargo.toml` |
| Rust target | `x86_64-pc-windows-msvc` | `.cargo/config.toml`、`rust-toolchain.toml` |
| MSVC tools | 14.50.35717；cl 19.50.35719.0 / link 14.50.35719.0 | `windows-toolchain.json` |
| Windows SDK | 10.0.26100.0 | `windows-toolchain.json` |
| Slint / slint-build | 精确 1.18.1 | `Cargo.toml`、`Cargo.lock` |
| winit / raw-window-handle | 0.30.13 / 0.6.2 | `Cargo.lock` |
| Release | opt-level 3、thin LTO、codegen-units 1、panic abort、strip debuginfo | `Cargo.toml` |
| CRT | `+crt-static`，不打包 CJK 字体 | `.cargo/config.toml` / Windows 字体 |

先安装 [rustup](https://rustup.rs/) 以及带 C++ x64 工具的 Visual Studio / Build Tools、上述 MSVC 和 SDK。RS-01 验证机器复用 Visual Studio Community 2026 18.0.2 的已有 C++/SDK；新机器可从 [Visual Studio 历史版本](https://learn.microsoft.com/en-us/visualstudio/releases/2026/release-history)取得对应的 Build Tools 18.0.2，安装 C++ x64/x86 工具与 Windows SDK 26100。较新安装器中的同名 14.50 组件可能已有不同补丁，仍须通过脚本核对文件版本。

```powershell
rustup toolchain install 1.99.1-x86_64-pc-windows-msvc --profile minimal --component rustfmt --component clippy
# 工程文件已锁完整 MSVC host 名称，无需修改全局默认或另设目录 override。
# 默认检查并构建两种渲染器：
./scripts/verify-rust.ps1 -SmokeTest
# 单独构建软件渲染 Release（跳过 fmt/clippy/test）：
./scripts/verify-rust.ps1 -Renderer Software -SkipChecks
```

脚本查找已安装的匹配 MSVC，显式选择 SDK 与 x64 链接器，并核对 compiler/linker 文件版本；缺少版本时失败，不使用最新版代替。`rust-toolchain.toml` 同时锁定版本和 `x86_64-pc-windows-msvc` host，验证入口检查 `rustc -Vv` 的实际 host；已有 GNU 全局默认也无需改变。可在 `rust/` 中通过 `rustup show active-toolchain` 和 `rustc -Vv` 确认。rustup 未在 PATH 中时，脚本也会检查 `%USERPROFILE%\.cargo\bin`。工具链安装和依赖首次下载需要联网；使用代理的环境应先按本机配置设置 `HTTPS_PROXY` / `HTTP_PROXY`，rustup/Cargo 不一定读取 Windows 系统代理。

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
# 直接打开固定样例列表：
./rust/target/artifacts/femtovg/velo-slint.exe --isolated --query=code
# 真实发现/搜索/启动使用独立目录与热键（不指向正式 Velo 数据）：
./rust/target/artifacts/software/velo-slint.exe --instance-id=rs10-test --data-dir=D:/VeloSlintTestData --hotkey=Ctrl+Alt+K
# 每次创建实际窗口，2 秒后检查并退出：
./rust/target/artifacts/femtovg/velo-slint.exe --smoke-test --diagnostics
# 独立执行真实 Slint 组件的合成按键/焦点断言，仍在 2 秒后退出：
./rust/target/artifacts/software/velo-slint.exe --input-smoke-test --diagnostics
```

`--smoke-test` 要求 HWND、原生重绘事件、640 × 540 逻辑客户区以及有不同像素的 snapshot；FemtoVG 额外要求 OpenGL 帧。失败或提前关闭返回非零退出码，脚本还设有 15 秒超时。它不测试真实按键、IME 候选框、全 DPI 兼容或内存预算；截图会触发额外渲染，不能在性能测量期间使用此模式。

`--input-smoke-test` 使用默认固定数据，通过真实 Slint 组件发送合成按键，验证 22 项焦点、导航、合成命令保护、面板返回和空结果断言；会改变查询和减少动画状态。`verify-rust.ps1 -SmokeTest` 分别运行窗口冒烟和这一独立输入冒烟。两种冒烟都不能替代微软拼音的实际合成输入记录。

标准入口还运行真实业务组件冒烟：自动生成 helper/快捷方式和独立数据，要求显式 ID、数据目录、测试查询与 `--business-smoke-test`；验证实际目录查询、可见图片、组件导航、刷新/显隐和离屏引用释放，后台保存浅/深截图后经正常生命周期退出。入口为 `scripts/test-business-ui.ps1`，不能把合成组件事件写成实际键盘或 IME 记录。

`--diagnostics` 记录 IME 事件类型/合成状态、窗口/输入焦点、生命周期、热键注册结果、编辑后的 UTF-8 字节数、搜索结果数量、固定导航 ID 与拖入数量，不输出输入内容或文件路径；不会关闭正式失焦隐藏。选择日志仅含固定样例 ID 和 `action=record_only`。拖放进入会话内预览，只检查文件类型、存在性和是否为文件，不读取内容或执行文件；权限、实机证据与待验项见 RS-07。

## RS-04 资源与输入采集

`--measurement-log=PATH` 开启独立、不可覆盖的测量日志，记录原生 HWND、重绘请求、可观察的 OpenGL 帧、原生/组件焦点和真实输入回调；不保存查询内容。写入使用容量 128 的后台队列，丢失事件会使采集记录失效。不能与 `--query` 或冒烟探针混用，避免把预设字符串当成实际输入。

`--measurement-stdio` 要求同时提供日志路径，通过继承的标准输入接收 `status`、`hide`、`show`、`warm-bottom`、`warm-top`、`clear` 和 `quit`。控制线程阻塞等待，没有重复计时器或监听端口；父采集器断开时退出。它自动启用隔离模式，保留同一个原生窗口并关闭隐藏时的 presented 动画状态。测量写入/控制线程仅在请求时建立；正常业务的后台线程见 RS-10 小节。

`--fixtures --measurement-log=PATH` 可以观察原固定模型的正常生命周期；旧输入采集脚本显式添加 `--isolated`，继续使用“输入 → 关闭 → Snapshot”流程。正常模式关闭会隐藏，需要使用底栏“退出”结束采集。业务模式也记录字符接收回调，但组件模型更新不等于结果帧已呈现；端到端计时按 RS-12 单独验收。

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

共享资源脚本使用 `-ApplicationKind Slint -ProcessId <PID> -WindowHandle <实测十六进制HWND>`，读取产物旁的 `build-identity.json`，不会对 Rust exe 执行 `go version -m`。增加每秒原始记录、私有工作集、线程/句柄及可用的 GPU 计数；null 表示不可采，不能按零统计。默认 ApplicationKind 仍为 Wails，旧采集入口继续可用。`DataMode=Fixtures` 保留 RS-04 的固定样例；业务模式必须显式提供 `-DataDirectory` 与 `-MeasurementLog`，记录实际 JSON/磁盘 PNG 指纹、扫描完成状态、目录数量和应用持有的图标缓存估算，拒绝丢失事件及扫描中的空闲样本。默认也拒绝扫描警告；本机已核对的部分发现结果可以显式使用 `-AllowScanWarnings`，此时必须有扫描期间发布新快照的证据，报告保留警告及例外标记，不能将失败后仅保留旧快照当作成功扫描。

### RS-12 完整业务采集

使用 `--measurement-business --measurement-stdio --measurement-log=PATH --instance-id=ID --data-dir=PATH` 可以通过继承的匿名管道驱动完整业务和正常生命周期。该模式必须指定独立 ID/目录，不允许与固定样例、隔离或冒烟模式混用。`show`/`hide`/`quit` 经过正常生命周期回调；额外的 `query-editor`、`query-chinese`、`settings`、`home` 和 `refresh` 仅用于明确标记的合成负载，不执行选中项，不保存设置。合成字符回调带 `synthetic=true`，延迟导出拒绝将其计为真实输入。正常运行不创建管道控制线程或测量日志线程。

```powershell
# 先运行完整检查，确保两种产物与当前源码匹配。
./scripts/verify-rust.ps1 -SmokeTest
# 官方 Wails v0.9.11 Release 校验；两种 Slint Release、源码/脏补丁、同一业务数据副本固化。
./scripts/prepare-rs12.ps1 -OutputDirectory .reasonix/rs12-new
# 1000/10000 项，共享 JSON 与固定时钟；结果一致后保存三轮内核/服务计时。
./scripts/measure-rs12-search.ps1 -OutputDirectory .reasonix/rs12-search-new
# 顺序运行；采样期间不构建、截图或运行其他基准。每条默认包含 30 分钟操作/空闲循环。
./scripts/measure-rs12-slint.ps1 -Renderer software -PreparedDirectory .reasonix/rs12-new -RunName software-first
./scripts/measure-rs12-slint.ps1 -Renderer femtovg -PreparedDirectory .reasonix/rs12-new -RunName femtovg-first
./scripts/summarize-rs12.ps1 -PreparedDirectory .reasonix/rs12-new -OutputPath .reasonix/rs12-new/resources-summary.json
# 人工桌面采集：正常业务生命周期，Wails 同时重定向数据并拒绝并行实例。
./scripts/start-rs12-session.ps1 -PreparedDirectory .reasonix/rs12-new -Edition wails -RunName wails-desktop
# 在另一采集终端运行；窗口状态由操作者先设置，扫描完成后才接受样本。
./scripts/measure-rs12-session.ps1 -SessionDirectory .reasonix/rs12-new/runs/wails-desktop -Phase visible -IdleEvidence '填写本次实际完成扫描与中英文/图标预热的依据'
# 普通 Slint 无探针对照；按实际窗口确认扫描和预热后，再运行上面的会话采集入口。
./scripts/start-rs12-session.ps1 -PreparedDirectory .reasonix/rs12-new -Edition software -RunName software-normal -Uninstrumented
```

`-CheckOnly` 仅验证管道、业务模式、资源字段和正常退出，不是资源验收。正式采集包含三种空闲状态各 3 × 15 秒、100 次合成显隐及 30 分钟循环后的复采；启动轮询是根进程的离散观察峰值，不能称为完整进程树瞬时峰值。CPU 位图/纹理估算与磁盘 PNG、Windows GPU 计数分别保存；Slint 内部纹理/字形缓存仍不可直接采。官方 Wails 的真实窗口操作、三份程序的暖启动/真实热键/独立冷条件，以及 DPI 转换负载需另采证据，不能由这些合成操作代替。当前进度见 [RS-12](../docs/baselines/2026-11-04-slint-rs12/README.md)。

`start-rs12-session.ps1 -Uninstrumented` 运行同一 exe 的正常业务模式，既无测量日志，也无匿名管道线程。此时 `measure-rs12-session.ps1` 通过 PID 和原生窗口类只读定位 HWND，使用显式 `OperatorBusinessConfirmed` 记录操作者的扫描/预热确认；无法获得的内存目录计数、应用缓存和扫描警告留空，不伪造观察结果。此入口只用于单列的无探针对照；完整自动矩阵仍要求实际业务事件。

2026-11-04 的[普通模式可见补采](../docs/baselines/2026-11-04-slint-rs12/normal/README.md)已通过：Software / FemtoVG 的 PrivateBytes 中位数为 26.32 / 55.03 MiB，各 3 × 15 秒有效样本，两份实例均由用户正常退出并完成清理。它们与初轮探针数据分组，不能用单轮差额归因探针成本；该检查点之后的隐藏态等补验见下一段。

同日[剩余验证续验](../docs/baselines/2026-11-04-slint-rs12/remaining/README.md)已补齐正常模式隐藏 30 秒/五分钟各 3 个样本：Software 26.12/26.12 MiB、FemtoVG 67.26/67.30 MiB；阶段累计 54 个有效空闲样本。两种 Slint 各有 20 个物理热键与 3 个独立进程暖启动的实际字符接收记录，包含人工输入等待、未证明首次可输入性能目标；默认网络检查未安装更新。四个系统图标的 Go/Rust 三轮离线冷应用缓存/磁盘命中与像素一致通过，CPU LRU 与 GPU 纹理分开。Wails 人工输入、独立冷条件、真实 DPI/帧呈现与完整树扫描峰值在用户暂离期间保留待验，未决定默认 renderer。

2026-11-05 [续验](../docs/baselines/2026-11-04-slint-rs12/followup-2026-11-05/README.md)补齐 Wails 三份实际字符暖启动上界及正常退出，慢样本与额外一次未完整采集启动单列；物理热键用户反馈闪现隐藏/不能输入仍未解决，采集窗错位未冒充产品根因。三份候选同 100 ms 的启动/扫描全树观察已归档，Software / FemtoVG / Wails 扫描窗内私有提交采样最大值约 24.74 / 56.26 / 254.01 MiB，采集开销、警告、首次间隙和模式差异均保留，不与空闲基线混用或决定默认 renderer。校准启动/热呼出、严格冷条件、真实 DPI 和帧呈现仍待验。

同日白天[桌面补验](../docs/baselines/2026-11-04-slint-rs12/followup-2026-11-05/desktop/README.md)追加两种 Slint 的实际 96/144/96 DPI、中文和应用图标/GPU 提供程序短时观察。首轮 FemtoVG 实际留在 96 DPI，四个标签与实际不符的阶段明确排除；复验原生 HWND 确认 144 DPI。三份实例正常退出，显示缩放恢复原值；Wails 对照开始前桌面工具被实体 Esc 停止，测试探针正常退出且没有样本。23 个三秒阶段不计作稳定空闲/帧耗时，工具文本不计作物理输入；严格冷启动按用户安排稍后进行。

输入记录以 TextInput 真正接收字符为结束边界，保存失败观察，复用原有延迟汇总。Computer Use 会激活窗口且有调度/操作等待，因此当前方法 `ComputerUseAssistedUpperBound` 只能证明观察到输入，不能据此宣称暖启动或热呼出预算达标。Software 没有可用的首帧 notifier，`first_redraw_requested` 只表示请求，不标为已呈现帧。采集检查可单独执行 `./scripts/test-slint-measurement.ps1`。

Windows 接入方式见 [RS-01](../docs/baselines/2026-10-28-slint-rs01/README.md)，固定界面见 [RS-02](../docs/baselines/2026-10-28-slint-rs02/README.md)，输入/动画与实际微软拼音记录见 [RS-03](../docs/baselines/2026-10-28-slint-rs03/README.md)。`cargo test` 在每种渲染器下各有 53 项：26 项原型/Windows 回归、19 项数据与提交/诊断测试、8 项 RS-09 差分及历史/固定事务测试。共享数据契约和完整验证结果见 [RS-08](../docs/baselines/2026-11-03-slint-rs08/README.md)；消息注入、纯状态测试及离线数据往返不能替代实际按键、多屏或 Shell 交互验收。

生命周期状态与 UI 适配位于 `src/lifecycle.rs`，Win32 热键/窗口边界位于 `src/platform/windows/`。`examples/rs05_desktop_fixture.rs` 是无产品数据的临时前台窗口，可用 `--fullscreen`、`--owner=十六进制HWND`、`--reserve-alt-space` 或 `--reserve-f23` 验证全屏、所属弹窗及冲突，Esc 退出。可在初始化构建环境后用 `cargo build --release --locked --no-default-features --features renderer-software --example rs05_desktop_fixture` 构建；详细步骤见 RS-05 记录。

RS-06 的 `desktop.rs` 在独立阻塞消息线程上管理托盘及实例唤起，`instance.rs` 管理初始化前的双锁与路径隔离，`geometry.rs` 使用有符号物理坐标、显示器工作区及原生 DPI 放置窗口。显示设置、DPI 和恢复事件回到 Slint 事件循环后合并处理；拖动期间延后约束，避免跨屏时被拉回旧屏。Explorer 重启后重新注册图标，通知区未就绪时最多重试十次，没有永久轮询。`examples/rs06_native_probe.rs` 无参数读取显示器、客户窗口和托盘注册状态；`--tray-left ID`、`--tray-menu ID`、`--taskbar-created ID`、`--environment ID` 与 `--move HWND X Y` 用于明确标记的原生消息/几何注入，不能替代实际指针、物理拔屏或休眠验收。构建方法与前一个 example 相同，替换 example 名即可。

托盘的 `ui/assets/velo.ico` 原样复制自 `build/windows/icon.ico`，包含 16/32/48/64/128/256px 图层，由 Rust 的 `include_bytes!` 嵌入。按通知区域所在显示器的 DPI 请求小图标尺寸（本机实测 16/20/24/32px），选择不小于目标的源图层后交给 Win32 解码缩放；替换或退出时销毁应用持有的旧 HICON。RS-02 的其他固定资源仍通过 Slint 编译器嵌入。

界面适配位于 `src/ui.rs`，固定数据位于 `src/fixtures.rs`，纯导航与输入规则位于 `src/navigation.rs` / `src/input.rs`，可选组件输入探针位于 `src/interaction_probe.rs`；主题与共用组件位于 `ui/components.slint`，页面位于 `ui/app.slint`。滑动使用 Slint Flickable 与固定宽度滚动条，不使用 Fluent 内置不可关闭的滚动条/复选框过渡。首页/列表有 240ms 内容入场、tile 有 360ms 淡入及 24ms 顺序延迟，另有悬停、按压和原型面板过渡；均为有限动画，完整业务设置/导入页在 RS-13 接入 220/280 ms 进入和 100 ms 离开过渡，进入即时交互、离开立即禁用子控件。设置齿轮、扫描与录制动效有独立组件记录，实际原生 Wails 对照差异见 RS-13。减少动画将时长和延迟归零，并直接显示最终透明度/位置/尺寸；正常业务模式通过 `preferences::reduced_motion()` 读取 Windows 系统偏好，固定样例可用命令行开关。正常业务的主题通过配置保存，查询保持会话状态。

正常运行使用系统 DPI，已完成 [真实 DPI 补验](../docs/baselines/2026-10-28-slint-rs03/native-dpi/README.md)。`SLINT_SCALE_FACTOR` 仅供开发时渲染比例预验：与原生 DPI 不同时，曾观察到微软拼音候选框偏移；真实系统 100%/150%/200% 下未复现。RS-06 在原单屏电脑同一实例上补验两种渲染器的 100%/125%/150%/200% 并恢复原 125%；后续双屏电脑补齐混合 DPI、负坐标、实际拔插、S3 恢复及输入，并保留该机原两屏 100% 配置，见 [G2 验收](../docs/baselines/2026-11-03-slint-rs06/g2-acceptance.md)。无前台权限的后台父进程请求激活时仍保留可见但未获焦点的旧观察；已验证的真实热键、托盘菜单和资源管理器二次启动均取得原生焦点，不将可见等同于可输入。

RS-07 的 `src/import_preview.rs` 将文件元数据检查放到容量 2 的按需工作队列，预览最多 256 项；40ms 单次计时器合并原生逐文件拖放事件，清空后拒绝旧 generation 的结果。`platform/windows/interaction.rs` 在至多一个临时 STA 线程上处理有真实 owner 的原生多选、菜单和确认框；UI 线程接收结果并恢复实际前台所属页面的焦点。原生交互期间暂时阻止应用显隐、设置切换和退出；取消后恢复正常行为。提权模式显示普通 Explorer 拖放边界提示并保留选择文件入口，没有放宽系统 UIPI 过滤。完整记录见 [RS-07](../docs/baselines/2026-10-29-slint-rs07/README.md)。

RS-12 后续的[暂离自动补验](../docs/baselines/2026-11-04-slint-rs12/completion-2026-11-10/remaining-auto/README.md)复用本轮冻结产品，以仓库锁定MSVC/SDK编译外部原生机器输入探针。三候选的暖启动/机器热键接收校准和六条件查询动画CPU/桌面帧已测，完整Go/Vue共享验证通过；机器与物理输入单列，不修改产品正常显隐。原人工冷字符9/9不代替新机器冷计时，新九份副本需未来独立启动周期，未自动重启/改登录项，RS-12与默认renderer/G4仍未放行。

2026-11-11 [RS-12 本机测量矩阵验收](../docs/baselines/2026-11-04-slint-rs12/completion-2026-11-11/README.md)完成，独立机器冷计时已补齐 9/9。Software/FemtoVG 各三份上界均在 500 ms 内，最近秩 P95 为 287.492/473.743 ms；Wails 三份较慢上界、FemtoVG 内存超预算和历史失焦原因缺证保持原状。此前全部原件保留，正常退出与资源清理通过，未改产品或冻结工具。RS-12 完成指本机测量交付，不表示所有预算/发行稳定性通过，默认渲染器及 G4 仍按 RS-13/14 推进。

## 许可与发行前要求

本工程自有代码沿用仓库 MIT。Slint 提供多种许可选择，本桌面原型使用 [Slint Royalty-free 2.0](licenses/Slint-Royalty-free-2.0.md)，在顶栏齿轮可访问的“外观与关于”面板保留官方 `AboutSlint` widget。Slint 依赖不受项目 MIT 覆盖。直接依赖 raw-window-handle 0.6.2 与 windows-sys 0.61.2 均为 MIT / Apache-2.0 双许可；嵌入 Fluent SVG 的 MIT 许可与来源见 [资源说明](ui/assets/README.md)。

发行时还需从锁定构建的依赖许可证清单收集完整第三方 notices/许可文本，并验证 Windows 10/11、OpenGL 驱动和系统 DLL 的最低要求。RS-14 已建立[内部 ZIP 与 CI 准备入口](../docs/rust-slint-release-readiness.md)，明确收集到的文本及许可缺口，尚未生成正式 Rust 安装包或放行发行。静态 CRT 也不意味着不依赖 Windows 系统组件。不要将两种渲染器的 exe 大小当成内存结果。

RS-13 已按[2026-11-13 本机验收](../docs/baselines/2026-11-07-slint-rs13/completion-2026-11-13/README.md)完成Windows11x64已测范围：两当前206输入exe的100/125/150/175%真实微软拼音/布局和两份跨屏复核通过，用户接受明确列出的外观差异。当前104静态/26组件/132矩形有效，原生设置进出7段满足原门槛，FemtoVG脉冲峰间隔1201.204ms。原失败/误差超限/缺帧不删除，不宣称唯一曲线或精确呈现。六实例/11完整采集正常退出，显示恢复100%、正式数据未变；200%不可选及其他环境保持未测，默认renderer/G4仍归RS-14。见[RS-13复测说明](../docs/rust-slint-rs13-testing.md)。

RS-14 的[本地发行工程续验](../docs/baselines/2026-11-07-slint-rs14/completion-2026-11-12/README.md)已补齐 Windows PE 版本/图标/manifest、版本来源同步、精确版本许可补充和固定 NSIS 内部安装器。当前两份构建各 206 项输入与完整检查/冒烟回执一致；两份 ZIP 拒绝测试、14 项实际 NSIS 场景、两条 Go/Rust 包级回退和旧版回归通过。测试仅在固定隔离目录执行、不写系统快捷方式/卸载项；正常每用户候选只编译未执行。远端 runner 数为 0、默认 renderer/G4 未定；命令与边界见[发行准备](../docs/rust-slint-release-readiness.md)。
