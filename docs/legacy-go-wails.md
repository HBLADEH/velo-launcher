# Go/Wails 历史版本

Go/Wails/Vue 旧版只保存在 [go 分支](https://github.com/HBLADEH/velo-launcher/tree/go)，分支指向 `ac2f51ad1317f4abffab2c548af251e48ce16d2b`，包含 v0.9.11 源码、前端与构建入口。旧标签、Release 和 Git 历史继续保留。

2026-11-17 主线改为根目录下的标准 Rust + Slint 工程，移除 Go 源码、Vue/Wails、旧构建、旧 CI 和迁移期比较脚本。完整构建与测试不再安装 Go/Node。迁移期新增 oracle 的最后状态保存在整理前提交 `cc65619`；如要重跑当年的 Go/Rust 差分，请在独立目录检出该提交。

当前测试保留已生成的合成 JSON 预期、静态拼音/Unicode 数据及来源许可，继续覆盖数据与排序兼容。`docs/baselines/` 记录原构建身份和当时路径，不改写成新构建的证据。

官方 Go v0.9.11 系统安装仍可由 Rust 安装器原位接管。兼容安装器名称、首次数据快照和安装目录中的 `.velo-rollback` 恢复逻辑继续保留；有原 Go 快照时可运行 `uninstall.exe /ROLLBACK`。用户数据不会因回退或卸载而删除。迁移操作见 [v0.10.0 说明](releases/0.10.0.md)，当前开发见 [开发说明](development.md)。
