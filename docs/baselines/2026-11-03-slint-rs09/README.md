# RS-09：搜索、历史与固定/常用首页

- 日期：2026-11-03（北京时间）。
- 状态：已验证（本机 Windows 11 x64，RS-09 内核/事务与固定界面接入范围）；下一项 RS-10，G3 未验收。
- 前置：[RS-08 数据契约](../2026-11-03-slint-rs08/README.md)；G2 已在已记录的 Windows 11 x64 环境放行。
- 范围：纯 Rust 搜索/首页内核、历史与固定项事务、Go 差分，以及固定样例界面的内核接入。真实应用发现、图标提取、Shell 启动、正式用户数据接线和完整设置仍按 RS-10/RS-11 迁移；G3 尚未验收。

## 实现与行为依据

`core/normalize.rs` / `search.rs` 在构建索引时完成名称归一化、词边界/首字母、中文拼音后缀和别名。查询按原 Go 的 exact/prefix/word/substring/fuzzy 等级、来源/固定/历史加权和稳定的名称/ID 次序取前 N 项；非空查询的 boost 限制在 ±180，精确名称额外加分。查询不触及磁盘，不转换拼音，不复制完整应用库；结果持有必要条目的 `Arc`。

为了保留多音字首读音和 Unicode 规则，静态表由锁定的 go-pinyin 0.21.0 / Go 1.25.6 Unicode 15.0.0 生成。词典覆盖 41,923 个码点；只读二进制表共 354,410 字节，不在进程中创建全量字典映射，无新增 Cargo 依赖。生成/校验命令、来源及 MIT/BSD 许可证见 [search_data](../../../rust/src/core/search_data/README.md)。没有新增常驻 Go 进程、WebView 或 HTTP 服务。

`core/history.rs` 保留次数、原时区/有效小数位时间戳、query→app、每应用查询记录上限和 Go 的年龄/分数上限。写入复用 RS-08 的串行原子提交，失败保留旧磁盘文件和旧 `Arc`，搜索读取已提交快照，无需等待磁盘。公开入口 `record_success` 仅供真实启动成功后调用；本阶段的成功/失败请求是离线模型，不把它当作已接通 Shell。

`core/catalog.rs` 合并调用方提供的已过滤应用、自定义覆盖项、显式固定和系统工具。首页保持固定顺序、最多 12 个常用项、固定项不重复出现在常用/工具中、不可用系统固定项不显示；首页学习不受搜索 history_weight=0 禁用。搜索分别排序应用/工具后稳定合并，同分时维持 Go 的应用优先规则。固定变更通过同一个应用库提交，包含原未知字段；200 项上限、重复固定、撤销固定和失败回滚均有验证。

主程序在窗口创建前以一个有限后台任务构建合成快照并回收线程，之后 UI 只持有固定显示数据。固定时钟/合成历史维持 RS-02 的首页导航顺序，搜索实际使用上述内核；说明与路径作为合成关键词，样例结果上限为 20。点击/Enter 继续只记录合成选择，不学习真实启动、不执行路径、不读取或保存正式业务文件。设置、托盘与导入预览的原有边界继续有效。

## 差分与明确差异

输入和 Go 生成的预期见 [rs09.json](../../../tests/fixtures/compat/rs09.json)，维护流程见[共享用例说明](../../../tests/fixtures/compat/README.md)。Go 侧直接使用 `search.Index`、`history.Store` 与私有 `Service` 实现；生产公开入口仅提取 `homeAt/searchAt` 的时间参数，仍使用当前时钟，没有重写一份服务预测算法。

- 9 组归一化输入；152 组搜索比较完整有序 AppItem、数量和分数，浮点绝对误差允许 1e-9，排序不设容差。
- 4 组历史场景，覆盖学习/空查询/失败取消不学习、未来时间、零时间年龄饱和、纳秒/非整毫秒/原时区、100 条上限和唯一最低次数淘汰。
- 4 组服务场景、12 次固定请求、16 个首页/应用库状态和 59 次服务查询；覆盖个人历史优先、固定与自定义独立、同 ID 覆盖、稳定工具合并、过期系统固定、仅固定应用撤销、stale pinned 标记及 null/空集合。
- 磁盘与并发测试验证历史/固定写失败、重启读回、并发写不丢更新、读者不等提交、未知字段保留和 200 项上限。

Go 在查询次数并列最少时按 map 遍历随机淘汰；Rust 在这些合法候选中按查询键排序选择，始终保留当前查询。Go/Rust 分别验证相同约束，不伪造唯一的 Go 预期键。Rust 另拒绝 i64 学习计数溢出并保留旧快照，避免损坏计数。除此之外，已覆盖场景未允许排名或字段差异。

## 验证与构建

汇总见 [validation.json](validation.json)；两份最终构建分别保留 [FemtoVG 身份](femtovg-build-identity.json)和 [Software 身份](software-build-identity.json)。HEAD 为 `4d9dbaf9acf5286d9961fae1e89cca908b5af9d7`，工作区包含前阶段及本阶段未提交修改；两份构建各 95 个输入指纹完全相同，已逐项核对与当前源文件及实际 exe 一致，不能仅以 HEAD 关联本轮构建。

| 检查 | 结果 |
| --- | --- |
| `./scripts/verify-rust.ps1 -SmokeTest` | 退出 0；fmt、两种 all-targets clippy / deny warnings、全部测试和 Release 通过 |
| Rust 测试 | 每种 53 项：26 项原型/Windows、19 项数据/提交/诊断、8 项 RS-09 差分与历史/固定事务；0 失败 |
| 窗口/输入冒烟 | 每种实际 HWND、重绘、640×540 和非均匀 snapshot 通过；FemtoVG 观察到 OpenGL 帧；每种 22 项真实 Slint 组件断言通过 |
| Go 对照与表 | 真实 Go oracle、静态表复算、152 次内核查询及 59 次服务查询等全部差分通过 |
| RS-08 回归 | 两种渲染器的 Go→Rust→Go / Rust→Go→Rust、12 ID / 30 配置及真实 NTFS 拒绝写入均通过 |
| `./scripts/verify.ps1 -SkipBuild` | 退出 0；测量工具回归、gofmt、绑定生成、lint、26 项前端测试、typecheck/build、go vet 和全部 Go 测试通过 |
| 资源清理 | 验证入口回收合成临时目录、ACL 和锁；截图用隔离进程已全部退出，无遗留测试进程/热键/托盘；未访问正式目录 |

本机为 Windows 11 专业版 Insider Preview `10.0.26340` x64；Rust/Cargo 1.99.1、MSVC 14.50.35717、SDK 10.0.26100.0、Slint 1.18.1，Go 1.25.6，Node 22.23.2 / npm 10.9.8。没有改动工具链、renderer features 或引入新 Cargo 包。

| 构建 | exe 原始字节 | SHA-256 |
| --- | ---: | --- |
| FemtoVG | 10,859,520 | `002E638CD7238D9BB4B8F762ABA7F83B27629B396822345E41DC26AD6FF7C03B` |
| Software | 11,361,280 | `1540ABB9552E438CDFE344780854C2FB6B76521475467D9EF8C0F4965A7053F1` |

## 界面检查与适用范围

以下为本轮 Release 的原始窗口截图，系统 100% DPI，`--isolated --reduce-motion`，640×540 逻辑/物理像素，查询使用 `--query` 预设。目视核对了首页分组顺序、选中/固定标记、长路径省略、深浅色、拼音结果和空结果。两种渲染器的拼音场景都显示唯一“记事本”。

| 场景 | 截图 |
| --- | --- |
| FemtoVG 浅色首页 | [home](screenshots/femtovg-home-light.png) |
| FemtoVG 深色 `jsb` → 记事本 | [pinyin](screenshots/femtovg-pinyin-dark.png) |
| Software 浅色 `code` → Code / Code Editor Preview，首项有固定标记 | [code](screenshots/software-code-light.png) |
| Software 深色 `jsb` → 记事本 | [pinyin](screenshots/software-pinyin-dark.png) |
| Software 深色无结果 | [empty](screenshots/software-empty-dark.png) |

Computer Use 第一次显式激活及重新观察后的点击均报 `failed to activate captured window`；停止继续原生输入，改为只读取预设查询窗口的截图。截图能证明渲染内容，不能证明实际输入；按键覆盖来自 22 项合成组件断言。本轮未新增真实键盘/IME、多屏、DPI、正常生命周期、动画录像或性能验收，不扩大历史 G2 的适用范围。

本阶段不重新采集内存、启动或热呼出性能，不将 RS-04 旧构建的资源结论标在本轮程序上。真实发现/图标/启动、实际启动失败不记历史的 Shell 接线及完整用户数据回归仍属后续阶段。
