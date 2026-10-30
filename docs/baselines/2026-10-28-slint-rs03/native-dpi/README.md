# RS-03：真实系统 DPI 补验

执行日期：2026-10-28（北京时间）。补齐 [RS-03 主记录](../README.md)中真实系统 100%/150%/200% DPI 的待验证项。

**结论：本机 Windows 11 / 微软拼音的真实系统 DPI 初验通过，RS-03 更新为已验证。** 两种渲染器在三个新增 DPI 下的六列布局、文字/图标、候选框定位、实际中文提交与列表末项滚动均通过。恢复原来的 125% 后，窗口与输入冒烟再次通过；没有遗留 `velo-slint.exe` 测试进程。

## 环境与身份

- Windows 11 专业版 x64，10.0.26300；当前测试显示器分辨率保持 **2560 × 1440**。
- 通过 Windows「系统 → 屏幕 → 缩放」实际选择 **125% → 100% → 150% → 200% → 125%**，最后恢复设置窗口的非最大化状态。
- 测试子进程移除 `SLINT_SCALE_FACTOR`，父进程环境在 `finally` 中恢复。使用的是原生系统 DPI，没有使用应用渲染比例覆盖或注册表模拟。
- 沿用同一 RS-03 最终构建，没有修改应用源代码或重新构建。两种 exe 的 SHA-256 每轮检查前与公开身份核对：

| Renderer | SHA-256 | 完整身份 |
| --- | --- | --- |
| FemtoVG | `669B23B54806473B78EC6CCAFC90F5A649CD3D5A61FDDB786246679E492D1FA4` | [femtovg-build.json](../femtovg-build.json) |
| Software | `D16FF164060EA6AD5B16C9E5764574D8092CE433563072D9DC406BA30CA9779A` | [software-build.json](../software-build.json) |

## 原生尺寸与自动检查

每次切换系统缩放后新建进程，分别运行两个 renderer 的 `--smoke-test --diagnostics` 和 `--input-smoke-test --diagnostics`，检查退出码、窗口实际渲染、原生 DPI、物理客户区及 640 × 540 逻辑尺寸。FemtoVG 另需观察到实际 OpenGL 帧。

| 真实系统缩放 | winit 原生 scale | 物理客户区 | 两种窗口冒烟 | 两种输入冒烟 | 详细事件 |
| --- | ---: | --- | --- | --- | --- |
| 100% | 1 | 640 × 540 | 通过 | 各 15 项断言通过 | [checks-100.json](checks-100.json) |
| 150% | 1.5 | 960 × 810 | 通过 | 各 15 项断言通过 | [checks-150.json](checks-150.json) |
| 200% | 2 | 1280 × 1080 | 通过 | 各 15 项断言通过 | [checks-200.json](checks-200.json) |
| 恢复 125% | 1.25 | 800 × 675 | 通过 | 各 15 项断言通过 | [checks-125.json](checks-125.json) |

合计 **16 次窗口/输入冒烟进程、120 项组件输入断言**通过。组件断言覆盖首页/列表导航与滚动、焦点、合成状态命令保护、面板返回和空结果，不冒充实际输入法事件。格式、clippy、9 项行为测试与构建检查沿用主记录已经通过的结果；应用源码未变，没有重复构建。

原生事件示例：

```text
event=native_window backend=winit renderer=femtovg scale=1.5 physical_width=960 physical_height=810
event=smoke_result passed=true backend=winit renderer=femtovg ... logical_size_640x540=true ...
```

校验同时要求原生 scale 与物理尺寸匹配目标 DPI，避免只看到请求值或缩放后的截图就判为通过。完整汇总见 [validation.json](validation.json)。

## 实际输入与界面

每个新增 DPI、每种 renderer 均单独启动普通诊断窗口：

1. 观察六列首页、中文长名称、缺失图标回退、顶栏/底栏与内层滚动边界。
2. 不点击搜索框，使用实际 `j` 按键建立微软拼音合成，观察候选框与输入光标的对齐。
3. 使用实际 Space 选词，确认输入框显示中文“就”，事件为 `Commit → input_edited（3 UTF-8 字节）→ Disabled`，没有条目动作。
4. 全选后用字面文本输入“系统”构造 13 项列表，按实际 Up 从首项循环到“环境变量”，确认选中项完整滚入内容区，底栏仍固定。该字面文本用于布局检查，不当作拼音合成证据。

| 场景 | 首页 | 真实候选框 | 中文提交后 | 列表末项 |
| --- | --- | --- | --- | --- |
| FemtoVG 100% | [截图](femtovg-100-home.jpg) | [截图](femtovg-100-ime.jpg) | [截图](femtovg-100-commit.jpg) | [截图](femtovg-100-list-bottom.jpg) |
| Software 100% | [截图](software-100-home.jpg) | [截图](software-100-ime.jpg) | [截图](software-100-commit.jpg) | [截图](software-100-list-bottom.jpg) |
| FemtoVG 150% | [截图](femtovg-150-home.jpg) | [截图](femtovg-150-ime.jpg) | [截图](femtovg-150-commit.jpg) | [截图](femtovg-150-list-bottom.jpg) |
| Software 150% | [截图](software-150-home.jpg) | [截图](software-150-ime.jpg) | [截图](software-150-commit.jpg) | [截图](software-150-list-bottom.jpg) |
| FemtoVG 200% | [截图](femtovg-200-home.jpg) | [截图](femtovg-200-ime.jpg) | [截图](femtovg-200-commit.jpg) | [截图](femtovg-200-list-bottom.jpg) |
| Software 200% | [截图](software-200-home.jpg) | [截图](software-200-ime.jpg) | [截图](software-200-commit.jpg) | [截图](software-200-list-bottom.jpg) |

六组实际输入均观察到正常候选框和提交，未产生 `fixture_chosen`；列表导航都记录为 `Up / index=12 / id=environment`。脱敏事件见 [ui-events.json](ui-events.json)。完整 Enter、Space、方向键、取消、翻页和中英切换矩阵仍参见主记录中的 125% 实机检查，本补验只扩展真实 DPI 初验。

所有公开图片直接保存 Computer Use 返回的 JPEG，没有裁剪、重采样或重编码。工具将各 DPI 窗口归一化到 640 × 540，因此截图不是原生物理像素图，不能用来衡量细线/字形栅格化的逐像素差异。原生物理尺寸以日志为准。候选框可以跨越应用窗口，截图右侧的候选条截取边界不等于系统候选框被应用裁剪。系统设置页含账户信息，没有保存为公开证据。

## 先前偏移的结论与边界

首轮“系统 125% + `SLINT_SCALE_FACTOR=1/1.5/2`”的预验曾观察到候选框偏移。此次真实系统 100%/150%/200%、无比例覆盖时均未复现：候选框锚定在输入光标下方，没有覆盖输入文本。关闭的是原生 DPI 的待验证项；开发覆盖模式的限制仍保留，不用本次结果宣称已修复该模式。

本次每档采用新进程验证初始布局与输入，不替代 RS-06 的运行中 DPI 变化、跨混合 DPI 显示器移动及工作区约束。Windows 10 和第三方输入法仍按实际环境补测；本次没有新增兼容范围、内存或延迟收益结论。

系统缩放和分辨率已恢复并核对，16 个自动测试进程及 6 个手工测试窗口均退出。下一项为 RS-04；公开文件校验见 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)。
