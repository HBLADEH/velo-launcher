# Rust 工程审视、README 与界面截图

2026-11-18（北京时间）。本轮审视当前工作区的工程边界、文档入口与旧技术栈残留，按历史 README 的品牌和阅读顺序重写中英文首页，并补齐当前 Rust 界面截图。

## 整理结果

- 根目录为单一 Cargo 应用，Rust、Slint、核心业务、Windows 平台层、测试和原生打包资源各有明确位置；工程调用链及线程边界补入当前工程说明。
- 维护目录没有 Go / Vue 源码或 Wails / frontend 构建入口。清理旧界面注释、失效路径和测量工具中的旧分支；移除已被系统安装验证取代的旧预览包回退测试。
- 升级互斥锁、系统接管 / 恢复、固化数据契约、搜索静态表及其来源许可继续保留。历史证据不改写成当前构建结果。
- 中英文 README 恢复品牌、界面预览、功能、安装、快速上手、实现原理、开发、状态与贡献的阅读顺序。当前功能与未发布功能分别标明，发行准备入口改为根目录命令，首次 Rust 发行检查点归档。
- 重新绘制中英文 SVG 流程图，生成器只用 Python 标准库；补充文档索引并更新 Issue 模板中的版本、渲染器和诊断说明。
- 12 张实机窗口截图使用独立演示数据和同一份 Software 构建，以原始像素保存为 PNG，并接入 README 与用户指南。图片与来源见[截图说明](../../images/README.md)。

## 当前验证

`./scripts/verify-rust.ps1 -Product -SmokeTest` 最终退出 0。Software 与 FemtoVG 各 117 项 Rust 测试通过，各 1 项需要专用 125 文件 Everything 端点的 opt-in 测试按条件跳过。格式、clippy、固化契约、NTFS 拒绝写入、原生发现 / Shell / 更新恢复辅助检查、PE 元数据、窗口、输入、业务及页面 / 重启冒烟通过。

两份构建各 224 个输入指纹与当前文件一致，使用各自唯一 renderer feature；两者源码输入集合一致。原文身份文件分别保存为 [Software](software-build-identity.json) 与 [FemtoVG](femtovg-build-identity.json)，成功回执见 [Software](software-verification.json) / [FemtoVG](femtovg-verification.json)，汇总见 [verification.json](verification.json)。源工作区未提交，不构成干净提交绑定的正式发行资产。

测量工具回归覆盖嵌套进程树、采样期间子进程变化、构建 / renderer 身份、输入边界及丢失事件；Schema 5 入口在独立预演实例上运行通过，只验证采集工具，不形成产品资源预算结论。PowerShell / Python 语法、GitHub YAML、项目清理与发行资产工具回归、23 份当前 / 新归档文档中的 400 个本地链接与锚点、生成图一致性和 `git diff --check` 通过。

中英文 README 已按深浅主题进行本地排版预览，图示与功能图没有缺失；英文浅色预览的 9 个本地图片全部加载，页面宽度未溢出。README 徽章继续反映远端工作流状态，本轮本地检查不改标为远端 CI 通过。

截图阶段另行使用独立 INI 的 Everything 真实 Query2 端点，查询仅含六项公开内容的示例目录；未运行自动套件中的 125 文件专项，不能把六项截图扩展为分页或性能验收。没有执行组合批量启动或系统安装器。

先前被用户中断的完整验证没有最终成功回执，未计为本轮通过。原始工具时间戳按 UTC 保留，文档日期和截图本地时间使用北京时间。

## 清理与适用范围

拍摄实例、临时热键对应的进程以及独立 Everything 客户端已关闭。正式 Velo 进程 24180 和原 Everything 服务进程 6440 仍在运行。

自动动作审批拒绝删除本轮创建的 `D:\VeloReadmeDemo20261117`，返回原因仅为 `blocked by policy`；目录保留在本机，没有改用其他删除手段。该目录包含演示配置、原生发现的测试索引、示例文件和隔离 Everything INI，不属于发行内容。

本轮没有重新验证 Windows 10、多屏、混合 DPI、实体 IME、UAC / 安装器或性能预算。0.11.0 仍未发布，未执行 commit、push 或公开发行。此前实机验收继续保持各自版本、构建身份与适用范围。
