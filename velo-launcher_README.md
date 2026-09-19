# velo-launcher

> **Velo** — Fast. Light. Ready.

`velo-launcher` 是一个面向桌面端的轻量级全局启动器（Launcher），目标是提供类似 Wox、Listary、uTools、Raycast 的快速呼出与搜索体验，但在第一阶段只聚焦一件事：

**用尽可能低的资源占用，在尽可能短的时间内找到并启动应用。**

项目采用 **Go + Wails** 开发，优先支持 Windows，后续视项目成熟度考虑扩展 macOS / Linux。

---

## 1. 项目定位

Velo 不是一个“大而全”的效率工具箱。

第一阶段不追求插件市场、AI、剪贴板、复杂工作流，也不追求替代 uTools / Raycast 的全部能力。

Velo 的核心定位是：

- 启动快
- 搜索快
- 占用低
- 界面简洁
- 操作路径短
- 后续可扩展

一句话描述：

> **Velo 是一个快速、轻量、克制的桌面应用启动器。**

---

## 2. 产品目标

### 2.1 第一目标

通过全局快捷键呼出 Velo，输入应用名称，快速找到目标应用并启动。

典型操作链路：

```text
Alt + Space
↓
输入 "code"
↓
Velo 搜索 Visual Studio Code
↓
Enter
↓
启动应用
```

从快捷键呼出到应用启动，尽可能控制在极短时间内完成。

---

### 2.2 核心原则

Velo 的设计与开发应始终遵循以下原则：

#### Fast

速度优先。

包括：

- 快捷键呼出速度
- 搜索响应速度
- 索引速度
- 应用启动速度
- UI 渲染速度

---

#### Light

资源占用优先。

重点控制：

- 常驻内存
- CPU 空闲占用
- 后台磁盘扫描
- 索引更新频率
- 前端运行负担

Velo 不应为了功能丰富而引入大量长期运行的后台逻辑。

---

#### Simple

默认体验保持简单。

主界面原则上只有：

```text
搜索框
搜索结果
```

避免：

- 冗余按钮
- 多层菜单
- 首页信息流
- 不必要动画
- 强制登录
- 云端依赖

---

#### Local First

核心功能应完全本地运行。

应用索引、搜索、历史记录等基础能力不依赖网络。

---

#### Extensible

虽然 MVP 保持克制，但底层结构需要保留扩展能力。

未来可以扩展：

- 文件搜索
- Web 搜索
- 系统命令
- Calculator
- Clipboard
- Plugin
- Script
- Custom Command
- Workflow

但这些功能不应影响第一阶段核心架构的轻量性。

---

## 3. 技术选型

### Backend

```text
Go
```

主要负责：

- 应用索引
- 搜索
- 排序
- 配置
- 快捷键
- 应用启动
- 系统 API
- 缓存
- 历史记录

---

### Desktop Framework

```text
Wails
```

原因：

- Go 原生后端
- 前端开发体验成熟
- 相比 Electron 更轻
- 便于调用系统能力
- 适合桌面效率工具

---

### Frontend

推荐：

```text
TypeScript
+
Vue 3
```

也可以使用 React，但项目初期建议优先保持前端简单。

UI 层只负责：

- 输入
- 展示
- 交互
- 动画
- 状态

搜索与索引逻辑尽量放在 Go 层。

---

## 4. 目标平台

### Phase 1

```text
Windows 10
Windows 11
```

优先支持：

```text
x64
```

后续考虑：

```text
arm64
```

---

### Future

根据项目成熟度考虑：

```text
macOS
Linux
```

平台相关能力必须尽量通过接口隔离。

例如：

```text
internal/platform/
    windows/
    darwin/
    linux/
```

禁止将大量 Windows 专用代码直接散落到业务逻辑中。

---

## 5. MVP 功能范围

第一阶段只完成以下核心能力。

### 5.1 全局快捷键

默认快捷键：

```text
Alt + Space
```

要求：

- 系统全局生效
- 快速呼出
- 再次触发可隐藏
- 快捷键可配置
- 尽量避免与系统快捷键冲突

---

### 5.2 应用扫描

Velo 启动后建立本地应用索引。

Windows 首期至少支持：

```text
Start Menu
Desktop
Program Files
Program Files (x86)
Windows Apps
用户自定义目录
```

重点扫描：

```text
.lnk
.exe
```

对于 `.lnk`：

应解析：

- 应用名称
- Target
- Arguments
- Working Directory
- Icon
- Description

---

### 5.3 应用索引

应用索引建议统一为：

```go
type AppItem struct {
    ID          string
    Name        string
    Path        string
    ExecPath    string
    Arguments   string
    IconPath    string
    Description string
    Source      string
    Keywords    []string
}
```

应用索引需要：

- 去重
- 缓存
- 增量更新
- 后台刷新

避免每次呼出 Launcher 都重新扫描磁盘。

---

## 6. 搜索系统

搜索体验是 Velo 最重要的模块之一。

### 6.1 搜索目标

输入：

```text
code
```

能够匹配：

```text
Visual Studio Code
```

输入：

```text
wx
```

可匹配：

```text
WeChat
```

输入：

```text
term
```

可匹配：

```text
Windows Terminal
```

---

### 6.2 搜索优先级

建议综合以下评分：

```text
Exact Match
Prefix Match
Word Prefix
Substring
Fuzzy Match
History Weight
Launch Frequency
Recent Usage
```

参考：

```text
score =
exact_score
+ prefix_score
+ fuzzy_score
+ history_score
+ frequency_score
```

---

### 6.3 性能目标

应用数量：

```text
1000
```

单次搜索建议目标：

```text
< 10 ms
```

正常使用场景尽量达到：

```text
1 ~ 5 ms
```

原则：

**搜索过程中不得进行磁盘扫描。**

搜索只能访问内存索引。

---

## 7. 排序策略

Velo 不应只做简单字符串匹配。

需要逐渐学习用户习惯。

例如用户输入：

```text
c
```

第一次：

```text
Chrome
Calculator
Code
```

如果用户连续多次选择 Code：

后续可以变成：

```text
Code
Chrome
Calculator
```

建议保存：

```text
launch_count
last_launch_time
query_history
```

最终评分：

```text
SearchScore
+
UsageScore
+
RecencyScore
```

---

## 8. UI 设计

Velo 主界面保持极简。

示意：

```text
┌──────────────────────────────────────────────┐
│  V   Search apps...                         │
├──────────────────────────────────────────────┤
│                                              │
│  > Visual Studio Code                       │
│    Windows Terminal                         │
│    WeChat                                   │
│    Chrome                                   │
│                                              │
└──────────────────────────────────────────────┘
```

---

### 8.1 主窗口

建议：

```text
宽度：640px
高度：自动
```

默认：

```text
居中
靠近屏幕上方
```

参考：

```text
Top ≈ Screen Height × 0.2
```

---

### 8.2 UI 风格

关键词：

```text
Minimal
Clean
Fast
Native-like
```

避免：

```text
复杂渐变
大量阴影
大面积透明效果
复杂动画
```

动画必须：

```text
短
轻
不影响响应
```

---

### 8.3 键盘操作

Velo 应尽可能实现完全键盘操作。

```text
↑
↓

选择结果

Enter

启动

Esc

隐藏

Ctrl + ,
打开设置
```

---

## 9. 窗口行为

Velo 默认：

```text
无边框
始终置顶
任务栏隐藏
失去焦点自动隐藏
```

按下：

```text
Alt + Space
```

执行：

```text
Show
Focus
Select Input
```

再次按快捷键：

```text
Hide
```

---

## 10. 图标系统

应用图标必须本地解析。

Windows 可以考虑：

```text
Shell API
SHGetFileInfo
IShellItemImageFactory
```

避免每次 UI 渲染重新读取图标。

建议：

```text
解析
↓
缓存
↓
前端使用
```

缓存目录：

```text
%LOCALAPPDATA%\Velo\cache\icons
```

---

## 11. 数据目录

建议：

```text
%LOCALAPPDATA%\Velo
```

目录：

```text
Velo/
├── config.json
├── index.db
├── history.db
├── logs/
└── cache/
    └── icons/
```

MVP 阶段可以先使用：

```text
JSON
```

后续数据量增加后考虑：

```text
SQLite
```

---

## 12. 配置

建议配置结构：

```json
{
  "hotkey": "Alt+Space",
  "max_results": 8,
  "theme": "system",
  "launch_at_startup": false,
  "search": {
    "fuzzy": true,
    "history_weight": 1.0
  }
}
```

原则：

配置文件需要：

- 可读
- 可迁移
- 有默认值
- 配置损坏时可自动恢复

---

## 13. 推荐目录结构

```text
velo/
│
├── app.go
├── main.go
├── wails.json
├── go.mod
├── go.sum
│
├── cmd/
│
├── internal/
│   │
│   ├── app/
│   │
│   ├── config/
│   │
│   ├── hotkey/
│   │
│   ├── indexer/
│   │
│   ├── launcher/
│   │
│   ├── search/
│   │
│   ├── history/
│   │
│   ├── icon/
│   │
│   ├── storage/
│   │
│   └── platform/
│       ├── windows/
│       ├── darwin/
│       └── linux/
│
├── frontend/
│   ├── src/
│   │   ├── components/
│   │   ├── stores/
│   │   ├── styles/
│   │   ├── App.vue
│   │   └── main.ts
│   │
│   └── package.json
│
├── assets/
│
├── scripts/
│
├── docs/
│
└── README.md
```

---

## 14. 核心模块

建议拆分如下。

### indexer

负责：

```text
扫描应用
解析快捷方式
去重
建立索引
刷新索引
```

---

### search

负责：

```text
Query normalization
Exact matching
Prefix matching
Fuzzy matching
Scoring
Sorting
```

---

### launcher

负责：

```text
启动 exe
启动 lnk
传递参数
ShellExecute
错误处理
```

---

### hotkey

负责：

```text
注册全局快捷键
修改快捷键
冲突检测
```

---

### history

负责：

```text
启动次数
最近启动时间
Query -> App
排序权重
```

---

### icon

负责：

```text
提取应用图标
缓存
读取
失效更新
```

---

### platform

负责：

```text
操作系统相关实现
```

业务代码禁止直接依赖大量 Windows API。

---

## 15. 启动流程

Velo 启动：

```text
Start
  ↓
Load Config
  ↓
Load Cached Index
  ↓
Register Hotkey
  ↓
Start UI
  ↓
Background Index Refresh
```

关键原则：

**不要等待完整扫描结束后再启动 UI。**

应该：

```text
读取缓存
↓
立即可用
↓
后台刷新
```

---

## 16. 搜索流程

```text
User Input
    ↓
Normalize Query
    ↓
Memory Index
    ↓
Matcher
    ↓
Scoring
    ↓
History Weight
    ↓
Sort
    ↓
Top N
    ↓
Frontend
```

---

## 17. 应用启动流程

```text
Enter
↓
Get Selected App
↓
Record History
↓
Hide Velo
↓
Launch App
```

注意：

建议先隐藏 Velo，再启动应用。

这样用户体验更自然。

---

## 18. 性能指标

Velo 的核心竞争力就是性能。

建议设定以下目标。

### 启动

冷启动：

```text
< 500 ms
```

目标：

```text
< 300 ms
```

---

### 呼出

热启动状态：

```text
Alt + Space
→
窗口显示
```

目标：

```text
< 50 ms
```

理想：

```text
< 30 ms
```

---

### 搜索

1000 个应用：

```text
< 10 ms
```

---

### 空闲 CPU

目标：

```text
≈ 0%
```

---

### 内存

MVP 目标：

```text
< 80 MB
```

理想目标：

```text
< 50 MB
```

不要为了追求绝对数字牺牲稳定性，但必须持续关注资源占用。

---

# Development Roadmap

## Phase 0 — Project Bootstrap

目标：

建立可运行项目骨架。

任务：

- 初始化 Go Module
- 初始化 Wails
- 初始化 Vue + TypeScript
- 创建目录结构
- 创建基础 CI
- 创建基础日志系统
- 配置 Windows Build

验收：

```text
wails dev
```

可以正常运行。

---

# Phase 1 — Launcher MVP

目标：

实现最基础 Launcher。

功能：

- Alt + Space 呼出
- 搜索框
- 扫描 Start Menu
- 展示应用
- 搜索应用
- Enter 启动

验收流程：

```text
Alt + Space
↓
输入 code
↓
Visual Studio Code
↓
Enter
↓
成功启动
```

做到这里，Velo 已经具备 MVP。

---

# Phase 2 — Search Engine

改进：

- Fuzzy Search
- 拼写容错
- Prefix 优先
- Ranking
- Search Benchmark

增加：

```text
go test -bench
```

---

# Phase 3 — Indexer

支持：

```text
Start Menu
Desktop
Program Files
Program Files (x86)
```

实现：

- 缓存
- 增量扫描
- 去重
- 后台刷新

---

# Phase 4 — App History

记录：

```text
Launch Count
Last Launch
Query Mapping
```

实现智能排序。

---

# Phase 5 — Settings

增加设置页：

```text
General
Hotkey
Appearance
Search
Index
```

---

# Phase 6 — Performance

重点优化：

```text
Startup
Memory
Search
Index
Icon
```

建立 Benchmark。

---

# Future

MVP 完成后再考虑：

```text
File Search
Calculator
Web Search
Clipboard
Commands
Plugins
Scripts
Workflows
AI
```

这些功能暂时不进入核心开发范围。

---

# Codex Development Guide

后续开发由 Codex 主导执行。

Codex 在实现功能时需要遵循以下约束。

---

## 19. Codex 原则

### 19.1 不过度设计

不要提前实现：

```text
Plugin System
Cloud Sync
AI
Workflow Engine
RPC Framework
复杂数据库层
```

除非当前任务明确要求。

---

### 19.2 优先完成闭环

每一个阶段都必须可以运行。

不要一次提交大量未完成模块。

优先：

```text
Small
Working
Testable
```

---

### 19.3 Backend First

核心逻辑优先放 Go。

例如：

```text
Search
Index
History
Config
Launch
```

Frontend 不应承担大量业务逻辑。

---

### 19.4 避免重依赖

新增 dependency 前需要考虑：

```text
Binary Size
Memory
Startup Time
Maintenance
```

如果 Go 标准库可以简单实现：

优先标准库。

---

### 19.5 Windows First

当前阶段优先保证：

```text
Windows 10
Windows 11
```

跨平台接口可以预留，但不要为了尚未支持的平台增加过多复杂度。

---

## 20. Codex 每次任务流程

Codex 执行开发任务时建议遵循：

```text
1. 阅读 README
2. 阅读当前代码
3. 确认已有模块
4. 给出最小实现方案
5. 修改代码
6. gofmt
7. go test ./...
8. 前端 lint / build
9. wails build
10. 总结变更
```

---

## 21. Codex 禁止行为

除非明确要求，否则不要：

- 大规模重构已稳定模块
- 随意修改目录结构
- 更换核心技术栈
- 引入 Electron
- 引入重量级数据库
- 引入云服务
- 引入账号体系
- 在核心搜索路径中进行网络请求
- 在搜索过程中实时扫描磁盘
- 添加与当前 Milestone 无关的功能

---

## 22. 代码质量要求

Go：

```text
gofmt
go vet
go test
```

前端：

```text
lint
typecheck
build
```

要求：

- 函数职责明确
- 避免超大文件
- 平台代码隔离
- 错误必须处理
- 不滥用 panic
- 日志级别清晰

---

## 23. 测试要求

重点测试模块：

```text
Search
Ranking
Indexer
Config
History
```

Search 必须有：

```text
Unit Test
Benchmark
```

示例：

```go
func BenchmarkSearch1000Apps(b *testing.B) {
}
```

---

## 24. Definition of Done

一个功能只有满足以下条件才算完成：

- 功能可运行
- 无明显错误
- 测试通过
- 不明显增加资源占用
- 不破坏现有功能
- 有必要的测试
- 有必要的注释
- README / docs 在需要时同步更新

---

# Release Strategy

建议版本：

```text
v0.1.0
```

Launcher MVP

```text
v0.2.0
```

Search + Ranking

```text
v0.3.0
```

Indexer + Cache

```text
v0.4.0
```

History

```text
v0.5.0
```

Settings

```text
v0.8.0
```

Feature Complete Beta

```text
v1.0.0
```

Stable

---

# Repository

正式项目 / GitHub Repository 名称：

```text
velo-launcher
```

产品展示名 / Brand：

```text
Velo
```

GitHub 描述：

> A fast and lightweight desktop launcher built with Go and Wails.

Topics：

```text
launcher
windows
productivity
golang
wails
desktop
search
app-launcher
```

---

# Branding

项目名称：

```text
velo-launcher
```

产品品牌：

```text
Velo
```

含义：

```text
Velocity
```

强调：

```text
Speed
Lightweight
Flow
```

推荐 Slogan：

> **Fast. Light. Ready.**

备用：

> **Find. Launch. Done.**

---

# Product Philosophy

Velo 不应该成为一个臃肿的软件平台。

它首先应该是一把非常快的工具。

当用户按下快捷键时：

```text
Velo 必须立即出现。
```

当用户输入内容时：

```text
结果必须立即出现。
```

当用户按下 Enter 时：

```text
应用必须立即启动。
```

如果未来增加任何功能，都必须问一个问题：

> **它是否会影响 Velo 的速度、简单性和轻量化？**

如果答案是会，那么这个功能就需要重新设计。

---

# License

建议开源协议：

```text
MIT License
```

如果未来存在商业化计划，可以在正式开源前重新评估许可证。

---

# Current Status

```text
Phase 0 — 工程骨架已实现（验证记录见 README.md）
```

当前任务：

```text
Phase 0 — Project Bootstrap
```

下一步：

```text
Go + Wails + Vue + TypeScript 基础工程已初始化，
完成 Phase 0 验证后，
然后进入 Phase 1 Launcher MVP。
```

