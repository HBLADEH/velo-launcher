# 当前验证记录

环境：2026-09-20，Windows 11 25H2 x64，Ryzen 9 9900X3D，WebView2 153.0.4234.32，Go 1.25.6，Wails 2.12.0。

## 已取得的证据

- 自动测试覆盖：配置默认值/旧字段补齐/损坏备份/未来版本保护；历史持久化、查询权重与衰减；增量扫描、去重、删除、取消；内存匹配/排序/拼写容错；快捷键冲突与重新绑定；保存失败恢复原快捷键；失败启动不写历史；图标复用、失效与 HTTP 路径限制；损坏索引恢复和并发缓存搜索。
- Windows 集成测试实际注册备用全局键、创建并解析临时 `.lnk`、从 Notepad 快捷方式提取图标；本机 32 个 packaged app 图标均通过读取测试。
- 桌面验证使用 computer-use 技能，在 `--diagnostics` 实例中实际搜索 `code`、按 Enter 启动 Visual Studio Code；历史文件记录 1 次启动与 query `code`，再次空查询时 Code 已排名首位。
- 在 Visual Studio Code 中按 Alt+Space 成功呼出 Velo；Ctrl+, 成功打开设置，保存深色主题后界面和 config.json 同步变化。验证结束已恢复 system 主题。
- 默认 `--background` 实例日志证明窗口隐藏且快捷键注册成功。任务栏隐藏/失焦/多显示器和重启恢复还需完成默认模式回归，不能用诊断模式代替。
- 实际索引：Start Menu 211、Desktop 6、Program Files 692、Program Files (x86) 724、Windows Apps 32，共 1665 项；最新缓存 1665 项均有本地 PNG 图标。
- 生产 Windows x64 构建成功，约 11 MB；Wails dev 已实际启动。Go vet、全部 Go 测试、前端 lint/typecheck/build 已通过。最新变更继续通过 scripts/verify.ps1 验证。
- 仓库已推送到 GitHub（`HBLADEH/velo-launcher`，MIT）。远端 CI 首次运行暴露并修复两个干净目录问题：Windows checkout 的 CRLF 触发 `gofmt -l`、缺少 `frontend/dist` 使 `//go:embed` 失败；修复后 `Windows build` 工作流（run 35459277743）全流程通过并上传 `velo-launcher-windows-amd64` 产物。
- 首个预览版已发布：[v0.8.0-beta.1](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.8.0-beta.1)（prerelease）。CI（run 35459917732）安装 NSIS 后以 `verify.ps1 -Installer` 构建并上传 `velo-launcher.exe`（11501056 字节）与 `velo-launcher-amd64-installer.exe`（6373121 字节）；两者的 SHA256 记录在 release 说明中。安装包未签名，未做安装/卸载端到端验证。

- 第二版预览版已发布：[v0.8.0-beta.2](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.8.0-beta.2)（prerelease，tag 指向 `5ad2dd4`）。CI 两轮通过：main push（run 35526837776）与 tag push（run 35526961854），后者以 `verify.ps1 -Installer` 安装 NSIS 后构建并上传 `velo-launcher.exe`（11548672 字节，SHA256 `645c7fb8cff69aa7c84c2cafd5efb391653f38db9ce459eac99f7678df9e41d6`）与 `velo-launcher-amd64-installer.exe`（6392250 字节，SHA256 `844b263b6632e9169ad1928ae84ecae9ed92c511aa6f81597b404cec0484fc3e`）；校验值同时写入 release 说明。安装包未签名，未做安装/卸载端到端验证。

## 2026-09-21 回归补充

- 历史写入改为独立串行写锁与已提交快照：阻塞磁盘写入时搜索评分仍可读取，写入成功前新历史不可见；20 次并发记录持久化后无丢失，自动测试通过。
- Alt+F4 统一走隐藏通知；显式退出或无可用快捷键时允许退出。关闭、失焦及重复隐藏有单元测试；新关闭流程尚未完成桌面端到端回归。
- 窗口按当前显示器 DPI 调整尺寸并限制在工作区内；小屏幕、负坐标副屏及高 DPI 几何测试通过，实际多显示器行为仍待验证。
- 自定义目录使用临时 exe 扫描夹具完成真实扫描集成测试，覆盖索引落盘、移除/恢复目录及删除后刷新；夹具不会执行。登录启动注册命令在隔离的 HKCU 临时键完成写入/删除/幂等测试，未修改用户实际 Run 项，真实登录行为仍待验证。
- 持续运行的诊断实例最新扫描为 1705 项；重复启动可重新显示已有实例。该实例报告 Alt+Space 被占用，不能作为注册快捷键后隐藏/呼出的验收证据。
- 验证脚本先生成当前 Wails 绑定，再执行前端类型检查与构建，最后执行 Go 检查，兼容干净检出的 dist 嵌入要求。
- 本批最终验证：`scripts/verify.ps1 -SkipBuild` 通过（绑定、lint、typecheck、前端构建、Go vet/test）；Windows x64 生产构建通过，输出 `build/bin/velo-launcher-check.exe`。
- 索引刷新提交与设置写入使用独立提交锁串行化，避免版本检查与索引发布之间插入配置变更；搜索读锁不覆盖磁盘写入。回归测试覆盖过期扫描不能替换磁盘/内存索引、并发设置保存后磁盘与内存一致、调用方修改目录切片不会修改服务状态。
- 图标提取不再自行清理缓存：新索引接受且成功落盘后才清理，并保留上一代索引图标供仍显示的结果使用。测试确认未接受的扫描保留旧图标、索引写入失败不会触发清理。

## 性能基线

| 项目 | 实测 | 口径 / 状态 |
| --- | --- | --- |
| 1000 应用搜索 | 0.014–0.069 ms/op | go benchmark；2–3 allocs/op；达 <10ms 目标 |
| 第一次全量提取图标 | 16.22 s | 后台；初次 UI 继续使用已有索引 |
| 有图标缓存的扫描 | 3.56 s | 1665 项，本机包含部分拒绝访问目录 |
| 缓存结果前端就绪 | 470 / 523 ms | 单次记录，从 Go 入口到 frontend-ready；需重复与真正冷启动测量 |
| GPU 开启、诊断窗口显示 | 448.88 MB WS / 217.89 MB private | 主程序+6个WebView子进程；不含被启动的Code；15秒样本 CPU 0.051% 整机 |
| GPU 关闭、后台窗口 | 411.13 MB WS / 153.75 MB private | 主程序+6个WebView子进程；15秒样本 CPU 0.4% 整机；仍需排查和复测 |
| 长时间运行的诊断实例（2026-09-21） | 368.48 MB WS / 152.00 MB private | PID 22844 与 6 个 WebView 子进程；15.43 秒样本 CPU 整机占比四舍五入为 0.000%；窗口可见性未独立确认，不作为默认后台行为验收 |

WS 是各进程工作集之和，包含重复共享页；private 是私有提交内存，并非物理常驻内存。两者均明确保留，不以只统计 Go 进程来代替完整运行时成本。内存目标尚未达成。

资源脚本现输出各进程内存明细，并比较采样两端的 PID 与启动时间。根进程无效时拒绝测量；采样端点进程集合变化时，`CPUSampleValid=false`、CPU 百分比为 null，应待进程树稳定后重测。端点采样不能检测完全发生在两次采样之间的短命进程，因此该方法用于稳定空闲期，不用于启动阶段 CPU 总量。上述长时间运行样本中，Velo 主进程 private 为 32.88 MB，其余约 119 MB 来自 WebView2。

## 2026-09-21 启动台、图标与托盘补充

- 托盘图标：`VELO_INTEGRATION=1 go test ./internal/platform -run TestTray -v` 通过，实际创建并移除了通知区域图标，重复 `Close()` 无副作用；沙箱实例日志为 `"msg":"window ready","visible":false,"hotkey_error":"","tray":true`，即后台模式下托盘与快捷键同时可用。
- 图标清晰度：提取改为 `IShellItemImageFactory` 并按 64 px 渲染，探针实测开始菜单快捷方式、`System32` 可执行文件与 packaged app 均返回 64×64；随后改为优先使用快捷方式目标程序，探针图片确认图标不再带 Shell 的快捷方式箭头。缓存签名加入版本号，本机 1220 项图标已全部重新生成，抽查首图为 64×64。
- 过滤与去重：本机索引由 1665 项合并为 1220 项（重名副本），再按辅助项过滤为可见 1162 项；加入 Program Files 两层层级限制后，缓存 455 项、可见 402 项（Start Menu 208、Desktop 4、Program Files 125、Program Files (x86) 86、Windows Apps 32），深层组件数量为 0；`7z` 从 3 条（`7-Zip`、`NVIDIA app`、`AMDInstallManager`）合并为 `C:\Program Files\7-Zip\7z.exe` 一条，`Visual Studio Code` 从用户与公共开始菜单两份合并为一条，缓存中仍保留 `7-Zip Help`、`EA app 更新程序` 等辅助项供关闭过滤后使用。单元测试覆盖词边界（`Helpdesk`、`UpdateTool` 不误伤）、中文子串、重名合并优先级与“同名不同目标保留”。
- 空格键启动：默认开启，设置项 `space_launch` 可关闭；配置测试覆盖旧文件补齐默认值与显式关闭的持久化。UI 底栏提示随开关变化。
- 端到端：在临时 `LOCALAPPDATA` 沙箱中构建并运行 `--background` 实例（不接触用户真实数据），日志为 `index refreshed apps=1162 duration_ms=15697`、`frontend interactive elapsed_ms=433`；另以 `--diagnostics` 实例截取真实窗口位图，确认候选列表只剩 7-Zip / MuMu / A HUB 等真实应用（`Git\usr\bin\[.exe`、`AccCheckConsole.exe` 已消失）、图标清晰无快捷方式箭头、底栏提示为 `↑↓ 选择 · Enter/Space 启动 · Esc 隐藏`；验证后已停止进程并删除沙箱、截图与临时构建产物。
- 未完成：托盘菜单与左键单击的桌面端到端点击、资源管理器重启后的图标恢复、高 DPI 与 200% 缩放下的图标观感，需要人工回归。
### 开发模式候选图标与设置加载（2026-09-21 用户报告）

- 现象：`wails dev` 下候选项图标全部退化为首字母占位符，且底栏缺少空格键提示、`Ctrl+,` 打不开设置面板。
- 根因一：dev 模式下前端页面由 Vite 直接提供，`/icons/*.png` 请求不会到达 Go 的 `icon.Handler`，而是落到 Vite 的 SPA 回退（实测返回 `200 text/html` 的 index.html），`<img>` 解码失败触发既有的 `@error` 清空逻辑，图标退化为占位符。生产构建由 `icon.Handler` 正常提供图标，不受影响，真实数据目录的 `index.json` 与图标文件核对一致（2191 个 PNG，抽查均存在）。
- 根因二：同一轮实测发现 dev 模式页面加载后 Wails 的 IPC 桥才注册 `window.go`，页面首个绑定调用 `GetSettings()` 过早失败（`catch` 到错误后被 `show()` 清空，界面无提示），导致 `settings` 一直为空：设置面板因 `v-if="settingsOpen && settings"` 无法渲染，底栏也缺少空格键提示。生产构建使用 WebView2 原生 IPC，不受影响。
- 修复：`frontend/vite.config.ts` 增加 dev-only 中间件，按与 Go 侧相同的规则（`%LOCALAPPDATA%\Velo\cache\icons`，`^[a-f0-9]{32}\.png$`）直接提供图标，不嵌入任何图标资源；`App.vue` 的设置加载改为带重试的 `loadSettings()`（最多 20 次 × 100 ms），设置面板打开前也会补加载。
- 验证：全部在临时 `LOCALAPPDATA` 沙箱内进行（复制真实索引后运行 `wails dev`，不接触真实数据目录）。修复前截图确认图标为字母占位符、`Ctrl+,` 无法打开设置；修复后截图确认图标恢复清晰、底栏显示 `↑↓ 选择 · Enter/Space 启动 · Esc 隐藏`，且 `http://127.0.0.1:5173/icons/<hash>.png` 返回 `200 image/png`（修复前为 `200 text/html`）。生产构建路径另经 `scripts/verify.ps1` 与沙箱实例截图验证，行为不变。
## 仍需完成

- 默认模式隐藏/失焦/重复呼出/多显示器回归，以及失焦与原生下拉框的交互。
- 快捷键到前端可输入的端到端延迟、重复缓存启动与冷启动分布。
- 降低 WebView2 常驻成本；后台可见性与空闲 CPU 新修复的复测。
- 索引刷新、快捷键回滚、历史和关闭流程的进一步并发/边界检查；本机无 C 编译器，`go test -race` 尚未运行。
- 登录启动真实登录回归；自定义目录设置页面的桌面端到端回归（后端真实扫描集成已通过）。
- 最终构建、文档逐项审计；本机 `build/bin/velo-launcher.exe` 被运行中实例占用时无法覆盖打包产物。

## 快速启动与候选排序（2026-09-22）

- 新增“已固定 / 常用应用 / 系统快捷”首页，常用应用结合 Velo 本地历史与来源排序；搜索保留精确名称优先，并对固定项、常见应用、英文别名及辅助程序分别加权。
- 新增手动导入与快速启动持久化：手动导入支持 `.exe` / `.lnk`，不执行导入文件；去重保留目标与参数语义，固定/移除不删除原文件。持久化文件在该轮后续改动中由 `quick-launch.json` 迁移为 `application-library.json`（见下）。
- `./scripts/verify.ps1` 通过：绑定生成、前端 lint / 类型检查 / 构建、Go vet / 测试、Windows x64 打包。
- 新增回归测试覆盖常用应用与辅助程序排序、别名、精确匹配、导入去重、无效输入、快捷方式参数、重启恢复、重新扫描保留、取消固定、写入失败回滚、手动图标不被清理、添加模式失焦行为。
- 1000 应用搜索基准约 0.016–0.081 ms，2–3 次分配（本轮运行数据，不代表完整启动或系统性能）。
- 使用独立临时数据目录、`--diagnostics` 验证原生首页显示、固定立即生效、`w` 候选排序、添加模式和文件选择对话框。此模式不用于证明默认失焦隐藏行为。
- 跨窗口拖放被界面自动化工具的目标窗口检查拦截，尚未完成端到端实测。Wails 文件拖放已启用，导入后端已通过测试；仍需手动核验资源管理器拖入与真实多文件选择后的完整流程。

## 自定义应用库（2026-09-22）

- 新增 `application-library.json` 统一保存自定义应用与首页固定，固定、添加、删除共用同一个提交锁，持久化成功后才替换内存快照；版本号不受支持时拒绝启动并提示升级 Velo，避免旧版本读新文件后覆盖。旧 `quick-launch.json` 的手动条目在首次启动时迁移一次，原文件保留为备份，之后以新文件为准。
- 添加入口三处：首页“＋ 自定义应用”、底栏“自定义应用”、设置 → 索引 → 管理自定义应用。支持一次拖入多个 `.exe` / `.lnk`、文件选择对话框、粘贴完整路径；管理列表内逐项删除，添加与删除均不执行导入的文件。
- 删除只移除自定义记录及它的首页固定，不删除原程序；若同一 ID 也在自动索引范围内，删除后回到自动索引的结果，取消固定不影响自定义应用的搜索。添加应用不自动固定。
- 自定义应用的图标写入同一个图标缓存目录，刷新清理时与索引项、固定项一并保留；重复添加同一目标与参数不会产生重复项，程序路径失效时启动会返回错误提示。
- 管理面板打开时通过 `SetImportMode` 暂停失焦隐藏，便于从资源管理器拖入；窗口隐藏时清除该标记，`app_test.go` 覆盖失焦与隐藏行为。绑定新增 `AddApplications` / `BrowseApplications` / `GetCustomApplications` / `DeleteApplication` / `SetImportMode`。
- 回归测试（`internal/app`）：旧文件迁移不复活已删除项、删除失败保留内存状态、自定义与自动索引共用一条结果、导入去重与无效输入、快捷方式参数语义、重启恢复、重新扫描保留、取消固定、写入失败回滚、自定义图标不被清理。
- `./scripts/verify.ps1 -SkipBuild` 通过：gofmt、绑定生成、前端 lint / 类型检查 / 构建、Go vet / 测试。
- 待人工核验：资源管理器真实拖入与多文件选择对话框的端到端流程（界面自动化工具仍被目标窗口检查拦截），以及删除后图标缓存的磁盘回收效果。

## 自动更新（2026-09-22）

- 新增 `internal/version` 作为唯一构建版本来源，启动日志、`Status.version` 与更新比较共用；`TestVersionMetadataMatchesBuild` 校验 `wails.json` 与 `frontend/package.json` 的版本是否与构建版本一致，避免发布时漏改。
- 新增 `internal/update`：读取 GitHub release 列表（项目只发布 prerelease，`/releases/latest` 会忽略预发布版本，因此不能使用），按语义化版本比较（数字段按数值，`0.10.0 > 0.9.0`；预发布低于同号正式版；`beta.10 > beta.2`；`rc > beta`），忽略 draft 与无法解析的 tag。
- 资产固定为 `velo-launcher.exe`、`*-installer.exe` 与 `SHA256SUMS.txt`。下载写入临时文件后原子替换，返回流式 SHA-256；进度通过 `update:progress` 事件上报。
- 安全边界：未签名程序的自动安装必须以发布页 `SHA256SUMS.txt` 的校验值为准，缺失或不匹配时拒绝安装并提示手动下载；测试覆盖该分支，且被拒绝时不留下任何下载文件。
- 安装方式自动判断：`platform.DirectoryWritable` 判断程序所在目录可写（便携版）时原地替换，否则下载安装包静默升级；安装版会尝试通过卸载注册表键回到原安装目录后再启动，避免重启到旧路径。
- 替换与安装由独立 PowerShell 脚本完成：等待 Velo 退出（占用时重试至多 180 秒）、覆盖失败也会重启原程序、完成后自删；脚本以 UTF-8 BOM + CRLF 写入，兼容包含中文的路径。`RunDetachedScript` 使用 `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` 且不继承控制台。
- 界面：设置新增“关于”标签（当前版本、自动检查开关、手动检查、发布说明、下载并安装、下载进度），窗口底部在发现新版本时显示版本按钮；`auto_check_updates` 默认开启，仅在启动 12 秒后检查一次并提示，不会在后台下载或替换程序。
- 回归测试（`internal/update`、`internal/platform`、根包）：版本解析与比较、release 过滤、资产与校验文件选择、服务器错误信息（含限流说明）、下载校验与临时文件清理、脚本内容与单引号转义、BOM/CRLF 写入、目录可写判断、缺少校验文件时拒绝安装、状态版本一致性。
- CI 新增一步生成 `SHA256SUMS.txt` 并随产物上传，供更新校验与发布说明使用。
- `./scripts/verify.ps1 -SkipBuild` 通过：gofmt、绑定生成、前端 lint / 类型检查 / 构建、Go vet / 测试。
- 待人工核验：真实 release 上的完整下载与重启替换、安装版 UAC 静默升级、无网络或触发 GitHub 限流时的界面提示，以及更新后托盘图标与快捷键的恢复。
