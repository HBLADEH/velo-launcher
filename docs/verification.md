# 当前验证记录

环境：2026-09-20，Windows 11 25H2 x64，Ryzen 9 9900X3D，WebView2 153.0.4234.32，Go 1.25.6，Wails 2.12.0。

## 2026-10-27 OPT-00 测量与回归基线（待验证）

首次真正冷启动观察已完成：系统 21:59:11 重启，实际登录启动项已停用、此前无 Velo 进程；冻结 v2 于 22:04:55 启动，观察约 15 秒仍未确认搜索框可输入，用户反馈与之相符。本次计为 1 次失败 / 0 次有效，冷启动中位数 / P95 空缺，不能使用 `frontend interactive=427 ms` 替代。22:05:14 已恢复原登录启动项，安装版 PID 26904 后台运行，用户输入文件指纹保持一致。详见[冷启动原始证据与累计统计](baselines/2026-10-27-opt00-cold-01/README.md)。首次启动焦点问题仍需修复，OPT-00 保持待验证。

用户授权后的准备：已通过系统 `StdRegProv` 暂时移除实际 `Run\Velo` 登录启动值，精确恢复 / 再停用的真实往返检查通过，系统登录枚举确认无剩余 Velo 入口。当前安装版保持运行，应用配置未改。采集器新增每次冷尝试后恢复原值的嵌套清理，采集及清理双故障回归通过。准备时刻为北京时间 21:56:06，等待下一次手动重启；本次仍不增加冷启动样本，见[处理及恢复步骤](baselines/2026-10-27-opt00-cold-preflight/after-disable/README.md)。

重启后冷条件复核：系统于北京时间 19:25:07 启动，安装版于 19:26:10 通过仍启用的登录启动入口以 `--background` 运行，因此拒绝将本次启动计为冷样本。没有启动探针或停止安装版；冻结候选与五份输入文件的元数据一致。实际注册表启动项与 `launch_at_startup=false` 配置不一致，已保存单项备份和待授权的临时停用 / 恢复步骤，见[冷启动预检](baselines/2026-10-27-opt00-cold-preflight/README.md)。原阶段校验清单保持不变，冷启动有效样本仍为 0。

计时补采：仅测试构建的输入就绪探针、交互采集与导出流程已通过默认回归和 `go test -tags latencyprobe ./...`。已取得 20 次正式真实按键有效记录，中位数 29.4425 ms、P95 51.9961 ms，均为含时钟 / 观察 / IPC 开销的保守上界；保留 4 次正式失败。另有 1 次有效真实按键校准，合并 21 次时为 29.5415 / 55.4893 ms。三次正式暖启动仅一次在 12904.2182 ms 确认可输入，另两次超时；用户确认首次启动需要点击搜索框，额外 7862.2436 ms 的手动焦点恢复单独归档。3 次无效自动按键、2 次更早的暖启动超时及采集器路径错误均保留。真正冷启动需独立重启，当前 Boot 拒绝检查及下一次采集准备已完成；测试版已关闭、用户输入文件未改写。见[计时原始记录与补采步骤](baselines/2026-10-27-opt00-latency/README.md)。

继续验证（10:27–10:41，北京时间）：[受控资源矩阵](baselines/2026-10-27-opt00-controlled/README.md)已通过，两份程序在相同配置 / 索引 / 历史 / 应用库、WebView2 版本和 DPI 下各取得三状态 × 三个 15 秒样本，合计 18 个有效样本。仅测试副本关闭自动检查更新以排除启动一次性网络工作。候选 / 安装版隐藏约五分钟的 PrivateBytes 中位数为 138.83 / 138.84 MiB；这是私有提交，不是物理常驻验收。结束记录确认用户输入文件未被测试改写，当次已按原参数恢复安装版。冷 / 暖启动与至少 20 次按键到可输入的计时仍无原始证据，总体继续“待验证”。

验证补充：当前代码再次通过 `verify.ps1 -SkipBuild`，安装版与候选已在相同数据副本上实际运行，新增 3 组、共 9 个采集器判为有效的样本；两者重扫索引 SHA-256 和 388 项可见数量一致。验证随后中断，稳态条件、隐藏完整矩阵与端到端计时未完成，继续保持“待验证”。原始数据及启动年龄 / 自动更新影响见[验证补充记录](baselines/2026-10-27-opt00-validation/README.md)。

本次使用 Windows 11 `10.0.26300`、WebView2 `155.0.4283.62`、DPI 120 / 125%，Go 1.25.6、Wails 2.12.0、Node 22.15.0；不沿用上方历史环境。工作区提交 `45dffab58c2fa17993be19da8f5b76f00f6a4575` + 本次测量 / 基准 / 文档改动，未修改产品运行逻辑。

- 资源采集器新增程序 SHA-256 / 固定版本 / 代码提交来源、工具链、配置与数据指纹、索引数量、窗口所属 PID / 显隐 / DPI、持续观察时长和逐样本有效性。私有提交与工作集仍分开保留；采样中进程变动、CPU 计数下降、窗口状态变化或数据 / 刷新证据变化均不得当作有效空闲对照。新增回归在 PowerShell 7 与 Windows PowerShell 5.1 通过，并纳入 `verify.ps1`。真实 PowerShell 5.1 探针发现并修复默认编码读取中文 UTF-8 JSON 的问题，新增中文无 BOM 和解析失败不输出索引内容的回归；最终一秒只读探针通过，不纳入 15 秒基线样本。
- 新增 `BenchmarkServiceHome` / `BenchmarkServiceSearch`，覆盖 100 / 1000 / 5000 项、无历史 / 有历史、固定项和系统快捷混排；与原有内核基准共 47 个场景，各运行 3 轮、每轮 300 ms。1000 项 Home 中位数为无历史 0.102526 ms / 有历史 0.742825 ms，5000 项有历史为 4.409899 ms；1000 项内核三轮范围为 0.014632–0.089399 ms、2–3 allocs/op。服务测试包含历史计算但不含 IPC / 渲染，不能代表端到端交互。
- `verify.ps1 -SkipBuild` 通过（测量回归、前端 lint / 25 项测试 / 类型检查 / 构建、绑定、Go vet / 全部 Go 测试）。独立 Windows x64 生产构建 `build/bin/velo-launcher-opt00.exe` 为 13,426,688 字节、固定版本 `0.9.10.0`，SHA-256 `a28e8e244fe1ee14c109c98c06e107da42fb4cead8e6422b857803713b19d6f8`。前端产物 441,001 字节。
- 运行采样来自已有安装版 `C:\Program Files\HBLADEH\Velo\velo-launcher.exe`，SHA-256 `f58a976138bddb9ae4d481e5bf64b785808b1c42716b137ba31d73328143a137`，通过发布清单关联到 `eda6a9f5bf76b766e6fe4adab8d6386e6bc1ab00`。缓存 458 项 / 最后完成刷新可见 390 项、每 30 分钟刷新、Go + 6 个 WebView2 进程。安装版与候选的提交不同，候选未运行，不能将这些数据作为候选验收。
- 安装版共 12 个约 15 秒样本，11 个有效：可见 2 个（private 170.29–171.31 MiB），持续隐藏观察 ≥30 秒 3 个（169.92–169.98 MiB），持续隐藏观察 ≥5 分钟 6 个（175.57–175.86 MiB）；另 1 个采样中发生显隐切换，标为状态未确认并排除。隐藏时长是连续观察下限，尚未控制新的隐藏动作；长时间隐藏组前有一次定时刷新，不能将不同组内存变化归因为隐藏策略。
- 三状态各 3 个有效样本、从新隐藏动作计时的约 30 秒状态、真正冷启动 / 暖启动及至少 20 次按键到可输入计时尚未全部取得。已保留无效样本和空白延迟 CSV，新增汇总器固定中位数 / 最近秩 P95 算法并按程序 / 环境 / 输入 / 方法分组，不用工具调用耗时或旧启动日志补位。

完整原始数据、资源结果、程序 / 输入 SHA-256、执行命令与剩余验证条件见 [OPT-00 基线记录](baselines/2026-10-27-opt00/README.md)。状态继续为“待验证”；本次未运行安装包、替换安装版或宣称完成 80 MB / 50 ms 性能验收。

## 已取得的证据

- 2026-10-17 修复更新检查遇到 GitHub API 限流时报错的问题：本机匿名 API 请求已复现 `403 API rate limit exceeded`，公开 `releases.atom` 与资产列表均返回 200。新增回归覆盖 403 / 429、限流冷却与恢复、预发布版本选择、发布说明纯文本转换、实际下载与校验、异常发布页及缺失资产。`verify.ps1 -SkipBuild` 全部通过，Windows x64 生产构建产物为 `build/bin/velo-launcher-rate-limit-fix.exe`。联网探针强制 API 返回 403 后，通过真实公开发布信息从 0.9.9 识别到 0.9.10，读取三个发布文件及有效 SHA-256 清单，0.9.10 自身不提示更新。未替换本机运行实例，未发布新版本；本机缺少 C 编译器，未执行 Go race 检测。

- 2026-10-16 发布 [v0.9.10](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.9.10) 预览版，标签指向 `eda6a9f`，移除主菜单右下角的退出按钮，并将网格中的应用与系统快捷统一按搜索排名展示。本地 `verify.ps1 -SkipBuild`、main CI [38780670315](https://github.com/HBLADEH/velo-launcher/actions/runs/38780670315) 和标签 CI [38780676169](https://github.com/HBLADEH/velo-launcher/actions/runs/38780676169) 均通过，包括 25 项前端测试、Go vet / 全部 Go 测试、程序与 NSIS 安装包构建。回归测试覆盖混合来源排名、跨行导航、布局切换保留选中项和实际启动目标。产物为 `velo-launcher.exe`（13431296 字节）、安装包（7010072 字节）及校验清单（186 字节）；两份程序的 Windows 固定文件版本均为 `0.9.10.0`，本地复算及三个 GitHub 资产 digest 均匹配，详见 `docs/releases/v0.9.10.md`。安装包未运行。

- 2026-10-14 发布 [v0.9.9](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.9.9) 预览版，标签指向 `77796c3`，修复空回收站报错及启动搜索清空时机。32/64 位 Go 原生回收站查询均已只读验证；已为空时显示单按钮原生信息提示，实测空格关闭后正常返回，截图见 `docs/images/empty-recycle-bin-notice.jpg`。普通与管理员启动均在指令发出后立即清空，失败仍报告错误，后续返回不会覆盖新查询。本地 `verify.ps1 -SkipBuild`、main CI [38441767288](https://github.com/HBLADEH/velo-launcher/actions/runs/38441767288) 和标签 CI [38441766847](https://github.com/HBLADEH/velo-launcher/actions/runs/38441766847) 均通过，包括 24 项前端测试、Go vet / 全部 Go 测试、程序与 NSIS 安装包构建。产物为 `velo-launcher.exe`（13432320 字节）、安装包（7010399 字节）及校验清单（186 字节）；两份程序的文件版本均为 `0.9.9.0`，本地复算及三个 GitHub 资产 digest 均匹配，详见 `docs/releases/v0.9.9.md`。原生信息提示验证未调用清空接口或删除文件，发布安装包未运行。

- 2026-10-12 发布 [v0.9.8](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.9.8) 预览版，标签指向 `37bdf39`，修复成功启动后搜索内容残留和呼出搜索框未聚焦的问题。原生窗口通过同步焦点通知恢复 WebView 输入，前端补充延迟激活重试与重复启动可见状态同步；设置、导入和右键菜单打开时保留其焦点。本地 `verify.ps1 -SkipBuild` 与标签 CI [38257241328](https://github.com/HBLADEH/velo-launcher/actions/runs/38257241328) 均通过，包括 20 项前端测试、Go vet / 全部 Go 测试、Windows 程序和 NSIS 安装包构建。产物为 `velo-launcher.exe`（13430272 字节）、安装包（7009040 字节）及校验清单（186 字节）；两份程序的文件版本为 `0.9.8.0`，本地复算及 GitHub 资产 digest 均匹配 SHA-256 清单，详见 `docs/releases/v0.9.8.md`。未运行安装包或替换本机程序，快捷键、托盘、管理员启动和重复启动的完整真实桌面交互仍待手动回归。

- 2026-10-03 发布 [v0.9.6](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.9.6) 预览版，标签指向 `44c14f7`，包含网格布局、候选项右键菜单、词首排序、更新代理与 MuMu 组件过滤。本地 `verify.ps1 -SkipBuild`、main CI `37125211789` 和标签 CI [37125214254](https://github.com/HBLADEH/velo-launcher/actions/runs/37125214254) 均通过。产物为 `velo-launcher.exe`（13414912 字节）、安装包（7001816 字节）及校验清单（186 字节）；文件版本资源为 `0.9.6.0`，本地复算及 GitHub 资产 digest 均匹配 SHA-256 清单，详见 `docs/releases/v0.9.6.md`。已用更新客户端联网验证：以 0.9.5 为当前版本可识别 0.9.6、两个程序与校验文件，线上清单匹配下载的 CI 程序；以 0.9.6 为当前版本不提示更新（使用已认证 GitHub 请求及本机系统代理）。未安装或替换本机正在运行的程序，真实 UAC 与完整升级流程仍待人工回归。

- 2026-10-03 修复 MuMu 搜索混入虚拟机后台组件：默认辅助项过滤会隐藏 Program Files / 自定义扫描目录中 `Hypervisor` 目录下直接发现的 exe，保留快捷方式与显式导入、固定的入口；过滤作用于已有缓存，关闭过滤立即恢复缓存中的组件。回归测试覆盖组件名称、目录大小写与边界、快捷方式、启动参数、缓存加载与开关切换。本机只读缓存检查及真实目录扫描后，`mumu` 均仅返回“#0 MuMu安卓设备”和“MuMu模拟器”；后者按既有去重规则保留桌面快捷方式。未写入用户索引或启动 MuMu。`./scripts/verify.ps1 -SkipBuild` 通过（前端 lint / 测试 / 类型检查 / 构建、Go vet / 全部测试）；未替换运行中的程序或发布版本。

- 2026-10-01 修复更新下载绕过 Windows 系统代理的问题：更新器优先使用对应协议的代理环境变量，未配置时读取当前用户启用的手动代理；支持按协议配置、系统代理例外和 `NO_PROXY`，每次请求重新读取代理状态。PAC/自动发现暂不支持。连接失败提示网络/代理检查及手动下载。回归测试覆盖代理优先级、例外规则、开关切换，以及版本检查、校验清单、程序下载与跨主机重定向统一使用代理。`./scripts/verify.ps1 -SkipBuild` 完整通过。本机同一 GitHub 校验清单地址直连在 8 秒探针期限内超时，通过系统代理正常返回；修复后的更新器在没有代理环境变量时，实际下载 v0.9.5 的校验清单和 `velo-launcher.exe`（13258240 字节），流式 SHA-256 与清单一致。探针已清理，未运行下载程序、未执行安装、未修改系统代理；修复尚未发布。

- 2026-09-26 发布 [v0.9.2](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.9.2) 预览版，标签指向 `5df157f`。标签 CI [36181223886](https://github.com/HBLADEH/velo-launcher/actions/runs/36181223886) 完整通过（前端 lint/类型检查/构建、Go vet/全部测试、Windows 程序及 NSIS 安装包）。本地 `verify.ps1 -SkipBuild` 通过；真实桌面与开始菜单合并扫描验证“异环”保留桌面入口且可搜索、“渔力全开.url”被排除；本机只读发现 39 个可用系统入口。CI 产物下载后复算 SHA-256，与校验清单一致；上传后 GitHub 资产 digest 再次匹配。发布包含 `velo-launcher.exe`（13233152 字节）、`velo-launcher-amd64-installer.exe`（6951337 字节）及 `SHA256SUMS.txt`（186 字节），校验值见 `docs/releases/v0.9.2.md`。程序的原始文件版本资源为 `0.9.2.0`。未运行安装包、未执行真实系统设置更改，企业电脑和安装后自动更新仍待实机验证。

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

- 第三版预览版已发布：[v0.9.0](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.9.0)（prerelease，tag 指向 `7e571c1`）。CI（tag push，run 35636042510）以 `verify.ps1 -Installer` 构建，新增的校验步骤生成 `SHA256SUMS.txt` 并随产物上传：`velo-launcher.exe`（11954688 字节，SHA256 `472285873fb6cfbae9c6db05c7f96987c19e8efa76d2ccf1ed9bf72c49ffb2d7`）、`velo-launcher-amd64-installer.exe`（6706692 字节，SHA256 `d27ad639780b3de927efa6b72138d3f74f33e36e22595108b21789f8449db544`）。三个文件由 `gh release create` 上传，说明见 `docs/releases/v0.9.0.md`。以 0.8.0 为当前版本的更新器联网探针读取线上 release，识别到 v0.9.0 及三个资产且大小一致；自动更新的真实下载与退出替换仍未人工回归，安装包依旧未签名。

- 第四版预览版已发布：[v0.9.1](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.9.1)（prerelease，tag 指向 `9fb375e`）。CI 两轮通过：main push（run 35759235263）与 tag push（run 35759235428），后者以 `verify.ps1 -Installer` 构建，校验步骤生成的 `SHA256SUMS.txt` 随产物上传：`velo-launcher.exe`（12000768 字节，SHA256 `d8a2175618f206740e00950d076d898bb19e829026d78ed00737f6b33fe939fb`）、`velo-launcher-amd64-installer.exe`（6718648 字节，SHA256 `ef7bb2800b8f9bbdc09eb8963a5d99bb9ffcce1030518faaa5d89f2faedd9dcc`）。取回产物后在本地复算 SHA-256，与 `SHA256SUMS.txt` 一致；三个文件由 `gh release create` 上传，说明见 `docs/releases/v0.9.1.md`。收尾阶段本地 `./scripts/verify.ps1 -SkipBuild` 通过（gofmt、绑定生成、前端 lint / 类型检查 / 构建、Go vet / 全部 Go 测试）。以 0.9.0 为当前版本的更新器联网探针读取线上 release，识别到 v0.9.1 与三个资产且大小一致，线上 `SHA256SUMS.txt` 的校验值与本地复算一致，0.9.1 自身不会被提示更新（探针经本机代理并使用已认证请求；该出口 IP 的匿名 API 请求会触发 GitHub 限流）。启动台界面样式、窗口焦点与主题同步的人工桌面回归仍未完成。

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

## 首次启动焦点修复（2026-10-28）

- 修复顶层窗口已有焦点、WebView 文档却未获得焦点的交接问题；首次显示等待原生和前端就绪，宿主已聚焦时产生真实焦点转换。
- 原生 Win32 回归在修复前失败、修复后通过；新增就绪顺序、后台启动、显隐优先级和缓存首页握手回归。完整检查、26 项前端测试、带探针 Go 测试及 Windows 生产构建通过。
- 一次新 WebView 配置、生产模式启动确认无需点击即可获得文档 / 搜索框焦点；不是重启冷启动或完整性能验收。跨应用快捷键额外验证因 computer-use URL 安全检查停止，未继续输入；原安装版和原数据状态已恢复。
- 程序身份、诊断状态、源码一致性、验证边界和恢复记录见[焦点修复证据](baselines/2026-10-28-focus-fix/README.md)。
