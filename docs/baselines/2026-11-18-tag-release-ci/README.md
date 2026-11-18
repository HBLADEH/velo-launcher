# 托管 CI 与 tag 发版验证

2026-11-18（北京时间），Windows 11 x64 宿主机。托管 CI 与正式桌面验收分开；tag 发版入口和前置配置见[发行准备](../../rust-slint-release-readiness.md#推送-tag-发版)。

- `actionlint`（仅放行已知的 `concurrency.queue` 版本差异）、PowerShell 解析及项目工具回归通过。临时 Git 仓库验证 annotated tag、事件提交、main 可达性、版本和审批；模拟 GitHub CLI 验证别名优先上传、远端 tag 变化、哈希错误保留草稿与拒绝覆盖已有版本。没有操作真实 tag 或 Release。
- 托管 CI 入口在本机以 Software、FemtoVG 分别运行成功，各 114 项 Rust 测试通过、3 项交互桌面测试过滤、1 项 Everything 专项默认跳过。使用本机 MSVC，不等同于 GitHub 托管镜像已经跑通。
- 首次完整验证的 FemtoVG 窗口冒烟失败：截图和 OpenGL 帧有效，`RedrawRequested` 为 0；同一个 exe（`D0065731274A94BD40D41B3C327F97A416F36AA65EE0D6D7A420703E61E9B202`）独立运行 4 次，复现 1 次。只在冒烟模式显式请求原生重绘，仍要求事件、截图和真实渲染结果。
- 修改后 `./scripts/verify-rust.ps1 -Product -SmokeTest` 退出 0：两种渲染器各 117 项 Rust 测试通过、1 项 Everything 专项默认跳过；Release、原生辅助检查与桌面冒烟通过。用两份最终 exe 再各重复 5 次窗口冒烟，10 次均通过且收到重绘事件。
- 用校验过 ZIP、版本与编译器哈希的 NSIS 3.13 生成两个内部候选包，exe 身份与各自回执一致；没有执行安装器。真实发行策略仍在创建目录前拒绝未获批准的 0.11.0。

最终回执各核对 228 项输入，工作区为 `cc656198b53ed049ba6abb13ff6f76738e247004` 基础上的未提交改动：

- Software exe：`79721CC4795E1B29360539399AF9A2C8872D1B520A9F4E4FEDE4639D5A70E1A3`；内部候选：`F3A72FF464520A73BBC47B0452D2E6A4A491EC4233D44E3760749051FC62EE5A`。
- FemtoVG exe：`169057A38136047DF3F49BDE8B2B8E29A2A4474BF7C2E9407B8FDF266488FA8B`；内部候选：`E7D93D4188F13D88D3FB384E57B13EF2B587658F0785280DCB86366F69F4711F`。

尚未合入远端 `main`，也未配置发版 runner 或运行真实 tag 发布；徽章需要远端新 CI 成功才会更新。本轮没有新增物理 IME、混合 DPI、安装升级或性能验收。
