# RS-03：键盘导航、IME 与动画

执行日期：2026-10-28（北京时间）。对应 [Rust + Slint 重构方案](../../rust-slint-refactor-plan.md)的 RS-03。

**状态：已验证（本机 Windows 11 / 微软拼音）。** 实现、两种渲染器的自动检查、125% 的完整输入矩阵，以及后续真实系统 100%/150%/200% DPI 补验均通过；补验后已恢复系统 125%。真实 DPI 下未复现开发比例覆盖模式的候选框偏移，详见 [真实 DPI 补验记录](native-dpi/README.md)。本项没有开始 RS-04 的资源测量或迁移产品业务。

## 代码与构建身份

- 分支 `codex/rust-slint-refactor`，HEAD `ac2f51ad1317f4abffab2c548af251e48ce16d2b`；沿用已有未提交工作区，没有提交或替换 Go/Vue 实现。
- 工具链、依赖与 features 延续 RS-02：Rust/Cargo 1.99.1、MSVC 14.50.35717、SDK 10.0.26100.0、Slint/slint-build 1.18.1、winit 0.30.13。没有新增 Cargo 依赖。
- 两份 Release 都只启用各自的 renderer feature，使用同一 winit 窗口后端。FemtoVG 实际观察到 `AfterRendering + NativeOpenGL`；Software 由独占 feature、实际重绘及非空白 snapshot 验证。
- 全部公开截图和实机事件采自下表构建。两份构建的源码指纹一致，覆盖整个 `src/`、`ui/`、嵌入资源、工具链和验证脚本；不能仅以 dirty HEAD 标识源码。

| Release | 字节数 | SHA-256 | 身份 |
| --- | ---: | --- | --- |
| FemtoVG | 9,960,448 | `669B23B54806473B78EC6CCAFC90F5A649CD3D5A61FDDB786246679E492D1FA4` | [femtovg-build.json](femtovg-build.json) |
| Software | 10,463,744 | `D16FF164060EA6AD5B16C9E5764574D8092CE433563072D9DC406BA30CA9779A` | [software-build.json](software-build.json) |

产物位于 `rust/target/artifacts/{femtovg,software}/velo-slint.exe`，未替换现有安装包。exe 大小不是进程内存数据。依赖许可沿用 [RS-01 清单](../2026-10-28-slint-rs01/dependency-licenses.json)，官方 AboutSlint 仍可从外观面板访问。

## 实现与边界

| 内容 | 当前行为 |
| --- | --- |
| 首页导航 | [navigation.rs](../../../rust/src/navigation.rs)按各分组分别生成六列行，与 Vue `choiceRows` / `navigateChoices` 一致；上下保持当前列，遇到短行落到该行末列，再从该列继续；左右按全部条目顺序循环；空组不占行 |
| 列表导航 | 上下循环；左右留给输入光标；Ctrl/Shift/Alt/Meta 组合键不作为启动器命令；键盘选中后，首页/列表都按最近边界滚动到可见位置 |
| Enter / Space / Esc | Enter 记录当前合成条目的 ID，重复 Enter 不重复记录，空结果没有动作；Space 保留普通文本语义。Esc 优先关闭外观面板，其次清空查询；空查询只消耗按键，正式隐藏与热键/托盘恢复留到 RS-05 |
| IME | [input.rs](../../../rust/src/input.rs)跟踪原生合成事件，结合 TextInput 的预编辑文本保护命令。winit 0.30.13 Windows 的 `Enabled` 对应合成开始，`Disabled` 对应合成结束；空 `Preedit` 和 `Commit` 不提前解除保护，避免候选确认键进入应用命令 |
| 输入编辑 | TextInput 保留选择、复制、粘贴、删除与光标编辑；过滤只处理提交后的文本，不用预编辑串切换页面；诊断不记录实际文本或路径 |
| 焦点 | 初始 `forward-focus` 指向输入框，显示后显式聚焦；原生激活后排队恢复焦点，并重新检查窗口可见/仍激活。面板显隐与焦点移交使用同一同步函数；面板打开时禁用后台可聚焦按钮，关闭后立即可输入 |
| 固定数据 | 仍为 3 固定项、12 常用项、13 系统快捷；简单子串过滤与记录选择，不调用 Shell、不读取用户数据、不执行系统操作。`--empty-pinned` 的首页仍按空组语义导航 |

UI 适配在 [ui.rs](../../../rust/src/ui.rs)，原生事件与探针入口在 [main.rs](../../../rust/src/main.rs)，页面和动画在 [app.slint](../../../rust/ui/app.slint) / [components.slint](../../../rust/ui/components.slint)。导航和命令规则可独立测试，不依赖真实窗口；扫描、历史、正式搜索、启动与持久化均未混入本项。

## 首批动画

参数取自 Vue 最终 Windows Blue 样式，统一缓动为 `cubic-bezier(.22, 1, .36, 1)`：

| 场景 | 参数 |
| --- | --- |
| 首页/列表内容入场 | 240ms，透明度 0→1、Y 位移 5→0px |
| 首页 tile 淡入 | 360ms，组内顺序延迟 `index × 24ms` |
| tile 悬停 | 外层上移 2px / 220ms，图标再上移 2px、尺寸 32→34.56px / 280ms |
| tile 按压 | 尺寸系数 0.96 / 220ms；交互命中区保持稳定 |
| 普通按钮按压 | 尺寸系数 0.96 / 180ms |
| 面板进出 | 入场透明度 220ms、Y 位移 8→0px / 280ms；离场透明度 100ms，离场立即交还键盘焦点 |
| 原有选中/悬停颜色 | 列表 140ms、按钮 160ms、tile/搜索边框 180ms |

两种渲染器使用相同的有限属性动画；尺寸/位置变化通过组件几何实现，不依赖额外 GPU transform 或持续计时器。减少动画将时长及顺序延迟归零，并直接使用最终透明度、位置和尺寸；从命令行或面板均可切换，尚未接 Windows 系统偏好。没有添加扫描旋转等无真实任务对应的循环动画。

过程帧与稳定截图分别保存，没有据此推算 FPS、输入延迟或空闲 CPU：

| 场景 | 过程帧 | 稳定画面 |
| --- | --- | --- |
| FemtoVG 面板入场 | [过程](femtovg-motion-panel-early.jpg) | [稳定](femtovg-motion-panel-stable.jpg) |
| Software 面板入场 | [过程](software-motion-panel-early.jpg) | [稳定](software-motion-panel-stable.jpg) |
| FemtoVG 列表入场 | [过程](femtovg-motion-list-early.jpg) | [稳定](femtovg-motion-list-stable.jpg) |
| FemtoVG 首页入场 | [过程](femtovg-motion-home-early.jpg) | [稳定](femtovg-motion-home-stable.jpg) |

另有 [FemtoVG 离场](femtovg-motion-panel-leave.jpg)、[Software 离场](software-motion-panel-leave.jpg)、[悬停状态](femtovg-hover-125.jpg)及 [FemtoVG](femtovg-reduced-125.jpg) / [Software](software-reduced-125.jpg) 减少动画开关。几何按压保留反馈，但不把它与完整 CSS transform 的文字缩放、阴影/光晕视为逐像素等价；完整视觉对照仍属 RS-13。

## 自动检查

执行入口：

```powershell
./scripts/verify-rust.ps1 -SmokeTest
node --experimental-strip-types --test frontend/tests/navigation.test.mjs
```

两种独占 renderer 均通过格式、clippy `--all-targets -- -D warnings`、9 项 Rust 行为测试、锁定依赖 Release 和真实窗口冒烟。另执行 Vue 原有导航的 2 项测试作为行为参照，本机 Node 22.15.0 需要 `--experimental-strip-types`。

新增独立 `--input-smoke-test`，由 `verify-rust.ps1 -SmokeTest` 在窗口冒烟后运行。它使用默认固定数据，给真实 Slint 组件派发合成事件，每种渲染器通过 15 项断言：初始控件焦点、首页循环与滚动、Enter 记录、输入过滤/重选、列表上下/水平编辑、合成状态命令保护、Esc 清空、列表循环与滚动、面板焦点移交、返回后输入及空结果无动作。探针位于 [interaction_probe.rs](../../../rust/src/interaction_probe.rs)，错误或提前关闭返回非零。

组件输入探针与实际 IME 分开记录，合成事件不冒充系统候选框/剪贴板/前台激活测试。[validation.json](validation.json)保留每个断言及窗口结果；原始检查日志在忽略的 `.reasonix/rs03/verify-all.log` 和 `rust/target/artifacts/`。

## 本机 125% 实机矩阵

环境为 Windows 11 专业版 x64，10.0.26300，微软拼音；原生客户区 **800 × 675 物理像素**，对应 **640 × 540 Slint 逻辑像素**。Computer Use 的物理按键用于拼音与导航，字面文本输入仅用于过滤和剪贴板样例。

| 场景 | FemtoVG | Software |
| --- | --- | --- |
| 正常启动，未点搜索框直接输入 | 通过 | 通过 |
| 首页跨组/短行/循环，选中项可见 | 通过；含上键到末项再下键回首项 | 自动组件路径通过；实际列表循环另验 |
| 列表循环、滚动、Enter 记录 | [末项](femtovg-list-wrap-125.jpg)、[记录](femtovg-enter-125.jpg) | [末项](software-list-wrap-125.jpg)、[记录](software-enter-125.jpg) |
| 实际拼音预编辑及候选框 | [截图](femtovg-ime-preedit-125.jpg) | [截图](software-ime-preedit-125.jpg) |
| 候选翻页与方向键选词 | [翻页](femtovg-ime-page-125.jpg)，应用选择未变 | [翻页](software-ime-page-125.jpg)，上下/左右未导航应用 |
| Space 选词，不触发条目 | [截图](femtovg-ime-space-commit-125.jpg) | [截图](software-ime-space-125.jpg) |
| 合成期间 Enter，不触发条目 | [截图](femtovg-ime-enter-125.jpg) | [截图](software-ime-enter-125.jpg) |
| Esc 取消合成，已有查询保留 | [截图](femtovg-ime-cancel-125.jpg) | [截图](software-ime-cancel-125.jpg) |
| 面板返回后不再点击搜索框即可输入 | [截图](femtovg-focus-return-125.jpg)；另验 Tab→关闭按钮→Enter | Esc 返回后直接选择/粘贴通过 |
| 中英混排选择、删除、复制与粘贴 | `系统 Code 中文` 全选/复制/删除/粘贴恢复，[截图](femtovg-text-paste-125.jpg) | 选择查询并粘贴同一合成剪贴板样例，[截图](software-text-paste-125.jpg) |
| 微软拼音中英切换 | Shift 切换后 `c` 直接过滤，切回后出现真实候选框 | 中文合成路径通过；普通字符编辑由组件探针覆盖 |

候选翻页后按方向键/Space 的实际事件没有产生额外 `keyboard_navigation` 或 `fixture_chosen`；Enter 提交产生 `Commit → input_edited → Disabled`，没有误记录条目。取消合成保留原查询，之后再按 Esc 才清空。脱敏事件见 [ui-events.json](ui-events.json)。

这里验证了普通启动、窗口已显示时的控件输入与面板返回。按键工具会激活目标窗口，不能由此宣称后台启动、Alt+Space 呼出、失焦隐藏及临时系统窗口的完整生命周期已通过；这些仍在 RS-05/RS-07。默认未开启 accessibility，UI Automation 只能看到窗口，因此结合截图、实际文本和原生诊断判定，未把窗口标题当作输入成功证据。

截图直接保存 Computer Use 返回的 JPEG，没有裁剪、重采样或重编码；125% 场景为工具归一化后的 640 × 540 图像。指针光圈来自系统/采集工具。截图不是原生物理像素栅格，也不用于宣称 RS-13 的 ≤2px 视觉差异。

## 首轮 DPI 比例预验与后续补验

首轮执行时真实系统缩放始终为 125%。当时尝试用 Computer Use 打开已存在的 `C:\Windows\ImmersiveControlPanel\SystemSettings.exe` 返回 `launched app did not expose a targetable window`，窗口列表也没有设置窗口，因此首轮仅完成比例预验并保留待验证状态。后续用户要求补齐真实 DPI 时，系统显示设置已可操作，实际切换与补验结果见下文。

首轮预验只在各测试子进程传入 `SLINT_SCALE_FACTOR=1/1.5/2`，退出后恢复环境，未改系统 DPI：

| Slint 渲染比例 | 原生客户区（物理） | FemtoVG | Software |
| --- | --- | --- | --- |
| 100% | 640 × 540 | [布局](femtovg-scale-100-home.jpg) / [候选框](femtovg-scale-100-ime.jpg) | [布局](software-scale-100-home.jpg) / [候选框](software-scale-100-ime.jpg) |
| 150% | 960 × 810 | [布局](femtovg-scale-150-home.jpg) / [候选框](femtovg-scale-150-ime.jpg) | [布局](software-scale-150-home.jpg) / [候选框](software-scale-150-ime.jpg) |
| 200% | 1280 × 1080 | [布局](femtovg-scale-200-home.jpg) / [候选框](femtovg-scale-200-ime.jpg) | [布局](software-scale-200-home.jpg) / [候选框](software-scale-200-ime.jpg) |

六列、字体、图标和固定内层滚动可渲染，候选框也能出现；但候选框未随 Slint 覆盖比例正确对齐光标，150%/200% 的预验中可见覆盖输入行。两种渲染器现象一致。它发生在“系统 125% + 不同 Slint 比例”的组合，本次只记录为比例覆盖模式的问题线索，尚不能判定真实系统 150%/200% 也有同样缺陷，更不能标记真实 DPI 通过。

后续已通过 Windows 系统显示设置实际切换到 100%/150%/200%，清除 `SLINT_SCALE_FACTOR` 后逐项重启两个最终 Release。每档均通过原生 DPI/物理尺寸校验、窗口/输入冒烟、候选框定位、中文提交、六列首页与列表末项滚动；恢复原来的 125% 后再次通过两种渲染器的窗口/输入冒烟。新增的 16 次窗口/输入冒烟、120 项组件断言、6 组真实微软拼音实测及截图见 [真实 DPI 补验记录](native-dpi/README.md)。真实 DPI 下未复现上述覆盖模式偏移，因此该待验证项已关闭。

第三方输入法与 Windows 10 未在本次环境验证，仍按用户实际环境补测。每档采用新进程验证初始尺寸与输入；运行中 DPI 变化、混合 DPI 多屏及工作区约束仍属 RS-06，不由本项的初验替代。

## 收尾与下一项

自动检查后进行了源码自查：导航例子与 Vue 测试一致；命令只在正确焦点/非合成状态处理；空结果和过期选择不执行动作；恢复焦点的迟到回调不显示窗口；动画无重复计时器。没有独立评审代理参与，不把自查写成独立评审。

未修改 Go/Vue 或共享采集工具，未运行 Go/Wails 打包及整套旧版回归。本次不包含内存、CPU 或启动/呼出延迟的达标结论，预算保持原案。测试窗口均正常关闭，收尾确认没有 `velo-slint.exe` 残留，没有注册热键、托盘或访问产品用户数据。

真实 DPI 验证已补齐，下一项为 RS-04 的采集适配、同机 A/B 资源样本及 G1 判断。公开证据校验见 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)。
