# Everything 文件搜索

Everything 文件搜索自 v0.11.0 起提供。首轮设计为 2026-11-16，实施续至 2026-11-17。初次验证范围见[本轮证据](baselines/2026-11-17-everything/README.md)。

## 使用与交互

设置 → 搜索 → 文件搜索中打开“启用 Everything”，保存后生效，默认关闭。可选填便携版程序路径，或点击“选择…”；留空时点击启动按钮会在标准安装目录和 PATH 中查找。需要在当前用户会话运行支持 Unicode Query2 的 Everything 默认实例，仅后台服务不足以提供查询端点。

Alt+Space 呼出后，在搜索框按 Tab 往返应用与文件模式，也可点击框内的“文件 Tab / 应用 Tab”。文件模式显示文件标识。两种模式分别保留查询与选择，第一次进入空文件查询可带入应用词。文件模式使用 Everything 原生语法，例如 ext:txt、folder:合同，不套用应用搜索的拼音、模糊匹配或历史权重。

文件固定用双行列表：名称、所在目录、类型及共用矢量图标。单击选择、双击或 Enter 打开，Ctrl+Enter 在资源管理器中定位。右键或 Shift+F10 提供打开、定位、复制完整路径，文件夹结果也可固定到首页或取消固定。文件模式的空格始终用于输入。Esc 依次关闭所属菜单、清空查询、返回应用模式、隐藏。IME 合成、设置页和原生弹窗继续优先处理其输入。

每次请求 50 项，用户明确加载下一页，最多保留 200 项。总匹配数与已加载数分开显示，达到上限提示缩小范围。新查询进行时保留当前窗口尺寸，避免列表尚未返回便收缩。隐藏窗口、进入设置或切走模式时释放结果，保留查询、选中路径和已加载范围；返回后重新查询并恢复仍存在的选中项。成功提交文件打开请求后回到应用模式并隐藏；失败保留原列表并提示。

## 配置与数据

config.json 增加 everything 对象：enabled 默认 false，executable 默认空字符串。旧配置读取时只在内存补缺省字段，不改写有效原文；显式保存继续使用现有串行原子提交、草稿/取消和失败回滚，保留根级及 everything 内未知字段。

FileHit 与应用 AppItem、应用搜索快照和使用历史独立；Slint 使用独立 FileEntry 模型。文件结果保存在当前会话中，显式固定文件夹时才会向应用库写入对应的首页固定记录。Velo 不建立第二份全盘索引，也不预读文件内容、提取 Shell 图标或缩略图。

## 原生实现与线程

[`src/platform/windows/everything.rs`](../src/platform/windows/everything.rs) 直接使用官方公开的 Unicode Query2 WM_COPYDATA 协议，按名称升序请求完整路径、类型和总数。SDK DLL、HTTP/ETP 服务及额外常驻代理均不进入 Velo 依赖。新增的 Windows API features 为现有 windows-sys 的 DataExchange、Memory 和 Shell_Common；没有新增 Cargo 包。

查询线程和消息窗口在第一次非空查询时创建。输入合并间隔为 80 ms，最多一项在途和一个可覆盖的最新请求。查询代次、当前模式、可见性和查询词共同拒绝迟到结果；原生回复 ID 与发送 HWND 另做校验。两秒期限覆盖本次 IPC 状态检查、发送和等待，过期或取消的回复不发布到界面。解析检查返回字节预算、数量、偏移、请求标志、排序、Unicode 长度/终止符及绝对路径。

隐藏、关闭页面或停用时撤销未完成任务，结果模型立即释放；空闲 30 秒退出查询线程并销毁所属窗口。停用立即请求退出，主线程保持响应；退出程序时在有界 IPC 完成后 join，避免遗留后台工作。应用索引刷新与图标结果不得覆盖文件模式。

Shell 打开、定位、启动 Everything 和打开官网均经过后台动作线程。定位使用绝对 PIDL 与 SHOpenFolderAndSelectItems，不拼接命令行路径。文件菜单、文件选择器遵循现有所属窗口焦点边界；配置保存成功后才切换运行状态。服务进程存在不会被解释为客户端已连接；未运行、索引准备中、零匹配、超时、协议错误分别处理。用户明确点击启动才运行已有 Everything 程序，不更改其索引或登录启动设置。

## 验证与边界

在仓库根目录运行 `./scripts/verify-rust.ps1 -Product -SmokeTest`，覆盖锁定工具链、两种渲染器、固化兼容基线及组件冒烟。Everything 专项 `./scripts/test-everything-ui.ps1` 另行运行，使用独立配置、125 个合成中文文件和用户提供的 Everything 可执行文件。专项覆盖分页、隐藏与设置往返恢复第二页选中项。遇到已有默认 IPC 客户端时拒绝占用，测试结束只清理自己的进程。

独立 Query2 测试默认跳过。准备好上述合成索引后，在仓库根目录按[开发说明](development.md#日常编译和预览)初始化锁定原生工具链，再显式运行：

```powershell
cargo test --locked --no-default-features --features renderer-software --lib live_query2 -- --ignored --test-threads=1
```

两种渲染器的具体 exe SHA-256、组件断言、原生交互和资源样本以各自的验证证据为准。多页选择往返见[专项复验记录](baselines/2026-11-18-review-followup/README.md)。

资源样本分别记录 Velo 与 Everything 的 PrivateBytes 和工作集，是带探针的组件运行记录，不是静默待机基准，也不能据此承诺所有设备内存下降。真实 IME 合成、其他 Everything 版本及多屏/DPI 未覆盖的项目单独记待验。文件预览、全文索引、文件收藏、批量文件管理、自动安装及多实例选择不进入首版。

## 来源与许可

官方来源为 [Everything SDK](https://www.voidtools.com/support/everything/sdk/)、[Query](https://www.voidtools.com/support/everything/sdk/everything_query/) 与[本机 IPC](https://www.voidtools.com/support/everything/sdk/ipc/)。
参考 SDK ZIP SHA-256 为 F5716D9513CCE6B462B5170A0A2E7E081E191D9BCAC6774F5DECC061497DF443。协议布局参考 `ipc/everything_ipc.h`，MIT 原文保存在 [licenses/Everything-SDK-MIT.txt](../licenses/Everything-SDK-MIT.txt)，来源为官方 `src/Everything.c` 顶部声明。应用关于页及现有许可打包入口包含该声明。独立 Everything 本体由用户安装，不作为 Velo 的发行资产。
