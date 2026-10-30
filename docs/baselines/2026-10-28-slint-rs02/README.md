# RS-02：固定首页、列表与主题

执行日期：2026-10-28（北京时间）。对应 [Rust + Slint 重构方案](../../rust-slint-refactor-plan.md)的 RS-02。

**结论：RS-02 已实现并完成本机界面验证，下一步为 RS-03。** 两种独占渲染器均完成格式、clippy、固定数据测试、Release 和真实窗口冒烟；本机 125% 缩放下，固定 640 × 540 逻辑客户区、六列首页、列表、主题与空结果均能渲染和操作。本记录不代表 G1，通过本项也不代表完整键盘/IME、Windows 生命周期、其他 DPI 或内存预算通过。

## 代码与构建身份

- 分支：`codex/rust-slint-refactor`；HEAD `ac2f51ad1317f4abffab2c548af251e48ce16d2b`，沿用 RS-01 的未提交工作区，没有提交或覆盖已有 Go/Vue 改动。
- Windows 11 专业版 x64，10.0.26300；本机缩放 125%。原生日志为客户区 **800 × 675 物理像素**，对应 **640 × 540 逻辑像素**。
- 工具链与 features 延续 RS-01：Rust/Cargo 1.99.1、MSVC 14.50.35717、SDK 10.0.26100.0、Slint/slint-build 1.18.1、winit 0.30.13。相同 Release 设置和 winit 后端，只改变 renderer feature。
- `src/fixtures.rs` 管理合成固定数据；`src/ui.rs` 在 UI 线程适配 Slint 模型/回调；`ui/app.slint` 承载页面，`ui/components.slint` 集中主题、tile、列表行、控件和滚动。没有新增 Cargo 依赖。
- `build.rs` 使用动态 Fluent 色板并嵌入资源；`verify-rust.ps1` 的身份扩展为覆盖整个 `src/`、`ui/`，含图标与许可文件。两个最终构建的源指纹一致。

| 最终 Release | 字节数 | SHA-256 | 完整身份 |
| --- | ---: | --- | --- |
| FemtoVG / OpenGL | 9,971,200 | `EEE1A2E49A190A0C4C27086F4B16418A75B9DA703F895E5349FBF6AE8C9E153F` | [femtovg-build.json](femtovg-build.json) |
| Software / softbuffer | 10,475,008 | `7C2DC44E86B42A2116E08E2F413BAAABA91E7D49FB10E7B376CD9593557DC077` | [software-build.json](software-build.json) |

产物仍在 `rust/target/artifacts/{femtovg,software}/velo-slint.exe`，不提交或替换安装包。源码和 exe 指纹描述本次 dirty 构建；HEAD 不能单独代表本次身份。文件体积不是运行内存结果。依赖及许可未改变，继承 [RS-01 许可清单](../2026-10-28-slint-rs01/dependency-licenses.json)；本次另外附带 [Fluent 资源来源与许可](../../../rust/ui/assets/README.md)。

## 交付与行为边界

| RS-02 要求 | 本次实现 |
| --- | --- |
| 固定窗口 | 无边框、置顶；640 × 540 逻辑尺寸的 min/max/preferred 约束；冒烟额外检查原生尺寸。底栏退出结束进程 |
| 搜索与页面 | 44px 搜索框，空白查询显示首页；输入按固定名称/路径/说明做大小写无关子串过滤，列表实时更新并回到顶部；清空可恢复首页 |
| 六列与分组 | 3 个固定项、12 个常用项、13 个系统快捷；每组重新从第一列排布，固定组与系统组末行不满六列；`--empty-pinned` 隐藏固定组但保留条目的可搜索性 |
| 文字和图标 | 中英名称、超过 20 字的中文名称、超过 85 字节的英文路径、缺失图标首字回退；嵌入本地 PNG/SVG，无网络、磁盘扫描或系统图标提取 |
| 主题与状态 | 浅/深 Windows Blue、默认首项选中、鼠标悬停更新选中、列表边框/文字高亮、空结果及清空入口；主题切换保留当前查询和页面 |
| 选择动作 | 点击仅显示“已选择：名称”并输出固定 ID 的 `action=record_only`，包括系统快捷；不执行路径，不读取/写入 `%LOCALAPPDATA%\Velo` |
| 减少动画与归属 | 齿轮打开外观与关于面板，主题和减少动画当场生效，展示官方 AboutSlint；仅保留 140–180ms 颜色过渡，可归零，无装饰循环动画 |
| 内容与滚动 | 固定 79px 顶栏/43px 底栏，内容区内层滚动；Slint Flickable 配固定 6px 滚动条，避免标准 Fluent 滚动条/复选框不可关闭的过渡 |

固定过滤不具备拼音、排序、历史或磁盘搜索语义，不能代替 RS-09。图标是复用的 Fluent 语义图标，不冒充各应用官方图标；正式应用图标缓存仍在 RS-10。减少动画暂未接 Windows 系统偏好；首页入场、悬停位移/缩放、按压以及完整焦点/合成输入保护由 RS-03 补齐。

标志原样复用现有 ICO 中的 128px PNG 图层；36px 显示在 200% 时为 72px，无需解码 1254px 原图。该固定资源 RGBA 尺寸由 6,290,064 字节变为 65,536 字节；只是资源尺寸说明，不是实测进程内存收益，也不替代缓存预算验收。

79px 顶栏、43px 底栏、22px 内容边距、7px 列间距、89px tile 和 54px 列表行沿用 Vue 最终层叠参数。固定 540 高允许内容滚动；没有把完整 660 高首页压缩。保留的 10px tile 文本、眉标、标题和分组标记均为当前 Velo 基线；本项不另建视觉风格。详见 [界面边界说明](design.md)。阴影、光晕和字体栅格化仍需后续同场景对比，不宣称已达到 RS-13 的 ≤2px 完整视觉验收。

## 验证与证据口径

主入口：

```powershell
./scripts/verify-rust.ps1 -SmokeTest
```

最终构建均通过：`cargo fmt --all -- --check`、两种独占 feature 的 clippy `--all-targets -- -D warnings`、3 个固定数据/过滤/过期选择测试、锁定依赖 Release、renderer features 校验以及 HWND/重绘/640 × 540/非空白 snapshot。FemtoVG 另观察到 `AfterRendering + NativeOpenGL`；Software 使用独占 feature 与实际图像证明。

另对每个最终 exe 顺序运行深色列表、浅色空结果、深色无固定组三个真实窗口场景；每项均验证退出码、预期固定结果数、原生尺寸和渲染结果，记录于 [scene-smokes.json](scene-smokes.json)。这些是预置状态冒烟，不能替代真实按键或 IME。

实机交互采用 computer-use 的窗口激活、点击、文本输入和滚动：中英文/大小写过滤、列表选择、主题切换、无结果清空恢复、系统快捷只记录、首页/列表末项可达。中文文本注入只证明 CJK 显示与过滤，不作为拼音合成输入证据。未改系统 DPI；100%、150%、200%、候选框定位/翻页/取消、方向键跨分组等保持 RS-03 待验。

截图为 Windows computer-use 原生窗口输出的 **640 × 540 归一化逻辑截图**，原生物理客户区另由日志保留；没有裁剪、合成或更改内容。指针光圈来自系统/采集工具。历史 Vue 图的逻辑高度/数据不同，因此不直接计算整图像素差异。所有公开截图采自上表最终 exe。

| 同场景截图 | FemtoVG | Software |
| --- | --- | --- |
| 浅色首页，默认固定数据 | [截图](femtovg-home-light.png) | [截图](software-home-light.png) |
| 深色首页，默认固定数据 | [截图](femtovg-home-dark.png) | [截图](software-home-dark.png) |
| `code` 浅色列表、长路径 | [截图](femtovg-search-light.png) | [截图](software-search-light.png) |
| `code` 深色列表 | [截图](femtovg-search-dark.png) | [截图](software-search-dark.png) |

补充状态：[首页系统快捷与末项](femtovg-home-bottom-dark.png)、[中文多结果](femtovg-search-cjk-dark.png)、[搜索列表末项](femtovg-search-bottom-dark.png)、[列表选择反馈](femtovg-search-selected.png)、[系统项只记录](femtovg-system-selected.png)、[减少动画与归属](femtovg-appearance-reduced.png)、[Software 无固定组及缺失图标](software-home-empty-pinned-dark.png)、[Software 系统快捷末项](software-home-bottom-dark.png)、[Software 中文结果](software-search-cjk.png)、[Software 中文选择反馈](software-search-selected.png)、[深色空结果](software-empty-dark.png)与[浅色空结果](software-empty-light.png)。无固定组截图与默认首页使用不同启动参数，不混作同一数据的 A/B。

最终运行的脱敏事件及 exe 指纹见 [ui-events.json](ui-events.json)。独立视觉评审发现并修正底栏分隔线位置，随后复核关闭；独立开放交互走查记录点击/清空可用，以及方向键/Enter 尚未实现的 RS-03 待办。评审范围、执行者和对应构建见 [review.md](review.md)。

格式/检查原始日志、交互日志、旧构建截图及独立评审过程保存在忽略的 `.reasonix/rs02/`，不提交真实用户数据。PowerShell 语法、两构建源指纹一致性及 `git diff --check` 已检查。没有改 Go/Vue 或共享采集工具，本项未运行 Go/Wails 打包与旧版全套回归。

验证结束已正常退出全部测试窗口，未遗留 `velo-slint.exe` 进程；没有注册热键、托盘或写入用户数据。公开文件校验值见 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)。

## 后续

下一项 RS-03：按现有 `navigation.ts` 语义实现方向键与循环选择，完成合成输入保护、首批入场/悬停/按压动画、真实焦点与微软拼音矩阵。RS-04 才适配资源/输入就绪采集并测两种渲染器的内存和延迟；本次没有性能收益或预算达标结论。
