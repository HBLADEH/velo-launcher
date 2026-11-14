# Go / Rust 共享行为用例

`rs08.json` 是合成数据，不含用户数据、真实查询或个人路径。`schema_version=1` 描述测试文件自身的结构，不替代各业务文件的版本号。

## RS-08 来源与范围

- `identities`：`internal/model/app.go` 的 `Identity`，覆盖路径大小写、参数 profile/参数大小写、中文/空格、Unicode 简单小写（Sigma、U+0130、Kelvin）、空值和 NUL 分隔。
- `configs`：`internal/config/config_test.go` 的缺省补齐、旧配置、显式关闭、枚举与范围校验；补充 JSON null、空数组、字段大小写、Windows 绝对/相对路径及无效类型。
- `bundle`：Go 的 `config.Config`、`history.Entry`、`indexer.Cache` 和应用库 JSON 结构。包含既有 ID、独立的自定义/固定记录、null/空关键词、UTC/正负时区、9 位小数时间戳，以及超出 JavaScript 安全整数范围的文件指纹。所有时间固定，不访问真实程序路径。
- `legacy`：`internal/app/library_test.go` 的一次迁移及删除后不复活语义。`internal/app/compat_test.go` 直接调用原 `loadLibrary` 验证私有应用库实现，避免只对照工具中的结构声明。

`expected`、`expected_bundle`、`edited_bundle` 和 `expected_legacy` 由当前 Go 基线生成。人工评审新增输入后，可在仓库根目录执行：

```powershell
go run ./cmd/rs08-compat refresh tests/fixtures/compat/rs08.json
```

常规验证只读取预期，不刷新：

```powershell
./scripts/test-data-compat.ps1 -Renderer Software
./scripts/test-data-compat.ps1 -Renderer FemtoVG
```

该入口构建离线 Go/Rust 对照程序，在全新临时目录执行 Go 生成 → Rust 修改 → Go 读取，以及 Rust 生成 → Go 修改 → Rust 读取；同时验证 NTFS 拒绝目录写入时四个原文件的 SHA-256 不变。测试只修改自己的临时目录 ACL，结束恢复 ACL 并清理。Rust 对照入口额外使用独立实例/数据锁，并沿用禁止产品目录及其子目录的检查。没有热键、托盘、扫描、Shell 启动或 HTTP 服务。

Rust 单测另验证磁盘原文备份、未知字段保存、损坏/未来文件保护、旧文件不重写、读者不等待提交、并发更新、只读文件、替换失败及 flush 后进程中断。Go 的类型序列化会丢弃未知字段；Rust 保存时保留它们是重构计划明确要求的改进，不能用 Go 的已丢弃字段输出作为保留性预期。

RS-08 只固化持久化结构；搜索、历史学习与首页的行为差分见下节，Shell 成功/失败后的真实接线留到 RS-10。

## RS-09 来源与范围

`rs09.json` 是独立的固定时钟合成 fixture：9 组 Unicode/空白归一化、152 组有序搜索、4 组历史学习/评分、4 组服务场景及逐步固定项变更。比较全部有序 AppItem 字段、结果数量和评分（Rust 浮点绝对误差 ≤1e-9）；不允许排序容差。查询包含装饰名称、精确/前缀/词内/子串、多词乱序、子序列、单编辑、中文/拼音后缀/首字母/别名、多音字的基线首读音、Unicode 简单小写、零/负 limit、极端历史加权和重复名称的 ID 次序。

历史覆盖次数/查询映射、空查询、失败/取消请求不学习、100 条上限、唯一最低次数淘汰、未来时间/零时间、原时区与非整毫秒时间。成功标记用于离线模型；它不证明真实 Shell 失败回调已接通。相同最低次数的 Go map 淘汰本来无确定键，另以 `TestRS09TiedHistoryEviction` 校验“只删一个旧最小项并保留当前查询”；Rust 选字典序首项，并单独验证溢出拒绝写入。

服务用例直接运行 `internal/app` 的原实现。`homeAt` / `searchAt` 仅提取固定时钟边界，公开入口仍调用当前时间；用例关闭尚属 RS-10 的噪声过滤，输入视为已过滤目录。覆盖 12 项常用上限、搜索历史权重为零仍学习首页、自定义覆盖同 ID、固定项移出常用/工具、不可用系统固定项、撤销固定后不保留仅固定条目、空/null 集合、固定标记和应用/系统同分时保留各自结果顺序。

人工评审新输入后生成预期：

```powershell
$env:VELO_RS09_REFRESH = '1'
try { go test ./internal/app -run '^TestRS09Behavior$' -count=1 }
finally { Remove-Item Env:VELO_RS09_REFRESH }
```

常规验证不会刷新预期；检测到此环境变量即拒绝执行：

```powershell
./scripts/test-search-compat.ps1 -Renderer Software
./scripts/test-search-compat.ps1 -Renderer FemtoVG
```

入口还用 Go 校验全量 Unicode/拼音静态表。标准 Rust 验证在循环前运行一次 Go 对照，然后每个渲染器的完整 `cargo test` 执行 Rust fixture、历史/固定持久化失败和并发测试，避免重复单独跑同一套 Rust 测试。

## RS-10 来源与范围

`rs10.json` 固化 37 组名称和 4 组发现/动作场景。Go oracle 直接调用未修改的 `indexer.IsNoise`、`Deduplicate`、`Visible` 与 `platform.ActionsFor`；Rust 比较全部有序 AppItem 字段和动作能力。覆盖桌面/开始菜单/其他快捷方式/打包应用/裸 exe 的优先级、别名去重、参数 profile、路径深度、同名不同目标、大小写/空白、Hypervisor 组件、显式入口和噪声过滤开关。

```powershell
# 人工评审输入后，才显式生成 Go 预期：
go run ./cmd/rs10-compat refresh
# 常规检查仅读取并验证预期：
./scripts/test-discovery-compat.ps1 -Renderer Software
./scripts/test-discovery-compat.ps1 -Renderer FemtoVG
```

`rust/tests/discovery_runtime.rs` 在自己的临时目录验证增量复用、修改/删除、桌面目标重检、深度、取消、独占目录导致的不可访问子树保留、PNG 复用/失效/损坏修复、命名空间清理、可见引用保护与 LRU 字节预算。服务单测使用真实 Windows 拒绝替换句柄验证保存失败不发布、不清图标；另验证旧扫描不能恢复已删除自定义项、失败/UAC 取消不学习及历史保存失败反馈。

标准 Rust 入口每种渲染器还运行 `rs10_windows` 原生验证：自动创建合成 `.lnk`/helper，与 Go 比较真实 COM 元数据、时间/大小/ID、Known Folders、AppsFolder 和旧 PNG 签名；只启动自己的 helper，检查参数、工作目录、成功/失败历史、重复请求、连续刷新、重启和锁隔离。本机真实应用列表仅在临时目录参与差分，公开报告只保留数量及通过项。发现和普通启动不调用 Go 或 PowerShell；RS-11 安装更新时使用一次性 Windows PowerShell 脚本。

`verify-rust.ps1 -SmokeTest` 额外为每个 Release 创建合成业务数据，通过 `--business-smoke-test` 验证实际目录搜索、可见图标、组件导航、刷新不覆盖新查询、隐藏不被迟到结果重新显示及模型图片引用释放，保存浅/深截图并正常退出。它使用真实业务后台及正常生命周期；按键属于 Slint 组件合成事件，不能代替真实 IME 或实际键盘验收。

## RS-11 页面、系统快捷与更新

`rs11.json` 固化 15 组版本比较（含非法版本）和 8 组 Windows build、协议、文件缺失及机器/用户设置页策略环境。`cmd/rs11-compat` 调用原 Go `update.ParseVersion` 和 `systemtools.Discover`，生成/检查 `rs11.expected.json`；Rust `pages_contract` 比较全部有序工具字段，并拒绝伪造来源或参数的协议动作。

`core/system_tools.json` 是从 Go 私有目录直接提取的静态数据；`internal/systemtools/rs11_compat_test.go` 防止两侧定义漂移。更新用例复用 Go 语义的数字/预发布排序；Rust 另覆盖草稿版本过滤、429/403 公开页面回退、限流期不重复请求 API、外部资产链接过滤、缺失/错误 SHA-256、下载中断/取消、已存在文件保护与脚本路径转义。

```powershell
go run ./cmd/rs11-compat
# 有意调整旧行为后，先审查差异，再更新预期：
go run ./cmd/rs11-compat generate
```

标准 Rust 入口也执行原生临时目录中的更新替换和失败恢复；`-SmokeTest` 再调用 `test-pages-ui.ps1`，为同一独立数据目录启动两次进程，检查七页浅/深截图、网格导航/合成保护、设置提交、固定、导入、备份、重启读回和删除。组件回调测试不代替实际原生菜单、UAC 或登录会话验收。使用 `-LegacyCopySource` 时只读复制源文件，测试结束核对源文件指纹不变，实际旧数据只保存在忽略的本地目录。

`rs11_windows` 的脚本启动使用真实 `canonicalize()` 路径，覆盖 Windows PowerShell 5.1 的入口路径转换。可选 `scripts/test-installer-update.ps1` 用 `rs11_installer_fixture` 在独立目录中实际运行生产更新脚本；分别验证管理员安装成功、注入失败后原 exe 不变并重启、配置/备份字节一致及资源清理。两种渲染器交替作为不同字节的原目标和 payload。此例程只在现有无提示提权策略下运行，不验证真实同意/取消对话框或正式 NSIS 包。

`rs11_startup` 在当前会话启动 `--background` 的真实程序副本。使用 10000 个合成缓存条目和独立注册冲突，覆盖读取保存快捷键前的首次显隐竞态、失败时窗口可访问及未来配置原文保护。程序与数据目录含中文/空格，没有登录项、键盘注入或真实登录会话。标准完整验证在各 Release 构建后运行此例程，原始文件保存在 `target/artifacts/<renderer>/startup-*`。

真实登录补验另发现两条命令超出 Windows Run 的 260 字符限制。`preferences` 原生注册表测试覆盖 UTF-16 边界、补充字符和拒绝后旧值不变；`service` 的事务测试覆盖已启用的旧配置再次保存时，超长命令失败保留配置原文、内存快照与原热键，并释放候选热键。Windows 登录后是否真正启动及能否输入仍由独立人工验收记录，不能由这些测试推断。
