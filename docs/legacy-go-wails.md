# 已停止维护的 Go/Wails 版本

自 Rust **0.1.0** 起，产品开发、构建和发布入口切换到 `rust/`。Go/Wails/Vue 的 0.9.x 系列停止维护，不再生成新版本。

仓库根目录的 Go 文件、`internal/`、`cmd/rs*-compat`、`frontend/`、`wails.json` 和 `scripts/verify.ps1` 暂留为历史行为及数据契约的测试基线。Rust 完整验证调用其中的离线 oracle；这不构成产品的 Go 或 WebView 运行时依赖。不要在这些入口继续开发产品功能。

旧版开发说明保存在 [历史开发文档](development-go-wails.md)，旧标签和 Release 保留原始内容。新开发命令见 [当前开发说明](development.md)。

官方 Go v0.9.11 系统安装可由 Rust 安装器原位接管，并保留字节校验的程序、注册和快捷方式快照。`uninstall.exe /ROLLBACK` 仅在存在原始 Go 快照时提供故障恢复；它不意味着恢复旧版维护。当前用户数据不会因回退或卸载而删除。
