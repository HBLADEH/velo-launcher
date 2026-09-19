# 实施与验收清单

目标范围：规划文档的 Phase 0–6；Future 明确不进入当前核心范围。每项以实际代码、自动化测试或运行记录验收，性能目标须实测。

| 阶段 | 交付 | 当前状态 / 验证证据 |
| --- | --- | --- |
| 0 | Go/Wails/Vue/TS、日志、Windows CI | 已实现：本地 gofmt/vet/test、前端 lint/typecheck/build 及 Windows x64 打包通过；远端 CI（windows-latest）执行同一脚本并上传 exe，已通过 |
| 1 | Alt+Space、Start Menu、内存搜索、键盘启动 | 核心链路已验证：Alt+Space 呼出、输入 `code` 后 Enter 启动 Visual Studio Code、Ctrl+, 打开设置；默认模式隐藏/失焦/多显示器回归未完成 |
| 2 | exact/prefix/word/substring/fuzzy、容错、排序、1000 应用 benchmark | 已实现，匹配/排序/拼写容错有单元测试；1000 应用 0.014–0.069 ms/op、2–3 allocs/op，达 <10 ms 目标 |
| 3 | Desktop/Program Files/Windows Apps/自定义目录、lnk 元数据、缓存/增量/去重/后台更新 | 已实现：本机索引 1665 项（Start Menu 211 / Desktop 6 / Program Files 692 / Program Files (x86) 724 / Windows Apps 32），增量、去重、删除、取消均有测试；自定义目录端到端回归未完成 |
| 4 | 启动次数、最近使用、query→app、本地持久化 | 已实现并有测试：实测写入 history.json，再次空查询时刚启动的应用排名首位；失败启动不记录 |
| 5 | General/Hotkey/Appearance/Search/Index、损坏恢复、登录启动 | 已实现：配置默认值补齐/损坏备份/未来版本保护、快捷键冲突与保存失败回滚均有测试；桌面实测设置页打开与主题保存；登录启动实际注册未验证 |
| 6 | 图标本地缓存、窗口行为、性能测量及优化 | 图标缓存已完成（1665 项均有本地 PNG）；搜索性能达标；内存实测 private 154–218 MB，80 MB 目标未达成；后台空闲 CPU 与呼出延迟待复测 |

横向验收：Windows API 隔离；搜索只读内存；缓存优先且 UI 不等待扫描；失败可见且不记录失败启动；快捷键冲突可恢复；设置与历史写入可靠；所有 Go/前端检查和 Windows 打包通过。

性能门槛：1000 应用搜索 <10ms；冷启动 <500ms（目标 300ms）；热呼出 <50ms；空闲 CPU 接近 0%；总常驻内存目标 <80MB。测量必须说明进程范围、环境与方法。

当前达成情况：搜索已达标；冷启动仅有单次 470/523 ms 记录，热呼出未测量，空闲 CPU 与内存未达标。口径与数据见 [验证记录](verification.md)。
