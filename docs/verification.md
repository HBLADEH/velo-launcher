# 验证入口

当前 Rust + Slint 构建、预览、完整验证和清理命令统一见 [开发说明](development.md)。

```powershell
./scripts/verify-rust.ps1 -Product -SmokeTest
```

此入口覆盖两 renderer 的格式、clippy、Rust 测试、固定数据/搜索契约、原生集成、Release、PE 身份与组件冒烟。Everything 真实 IPC 使用独立 Everything 和数据目录单独检查，入口 `scripts/test-everything-ui.ps1`。真实 IME、Shell、UAC 与多屏验证按相应验收范围记录，不能用合成事件代替。

README 的远端 CI 可以在本地用 `./scripts/test-rust-ci.ps1 -Renderer software` 和 `-Renderer femtovg` 等价检查其命令和测试范围；本机 MSVC 不代表 GitHub 镜像的 MSVC。发布使用完整锁定验证，见[发行准备](rust-slint-release-readiness.md#推送-tag-发版)。项目工具回归使用临时 Git 仓库和模拟 `gh`，不会推 tag 或操作真实 GitHub Releases。

终审修复后的双渲染器验证、Everything 分页专项与候选打包见[复验记录](baselines/2026-11-18-review-followup/README.md)。README 截图仍对应[上一轮工程审视构建](baselines/2026-11-18-project-review/README.md)，不改标为修复后的结果。

历史证据见 [基线索引](baselines/README.md)。旧 Go 验证入口位于 [历史分支](legacy-go-wails.md)，不再属于主线构建。
