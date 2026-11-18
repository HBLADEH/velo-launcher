# Velo v0.11.0 正式发行

2026-11-18 17:15（北京时间），GitHub 将 `v0.11.0` 公开并标为最新版本。注释 tag 绑定干净构建提交 `02fbc1e4b88dc10aaeb26e6c2fad7bfd38e8df6f`。[发布页](https://github.com/HBLADEH/velo-launcher/releases/tag/v0.11.0)

- `main` 的首次托管 CI 在 Rustup 解析带目标平台后缀的锁定工具链时退出。显式安装该工具链后，Windows 托管镜像上的 Software 和 FemtoVG 检查均通过。README 徽章返回 `passing`。[CI #69](https://github.com/HBLADEH/velo-launcher/actions/runs/44271587807)
- 本机对该干净提交执行 `./scripts/verify-rust.ps1 -Product -SmokeTest`，双渲染器通过各 117 项 Rust 测试及完整桌面冒烟。Windows 11 x64 的完整检查及正式打包回执使用锁定的 Rust、MSVC、SDK 和 NSIS。本轮安装器验证范围为打包和产物校验。
- 发布时仓库未配置专用 runner，使用本地验证与草稿预上传路径。tag 注释绑定 `SHA256SUMS.txt`。Actions 下载草稿、核对六项资产与构建提交后完成发布。[发布运行 #1](https://github.com/HBLADEH/velo-launcher/actions/runs/44274096501)
- 公开后的六项资产 SHA-256 与本地文件一致，公开下载的 `SHA256SUMS.txt` 也与 tag 一致。首个 `.exe` 为 Software 兼容别名，字节与默认安装器相同。[校验清单](https://github.com/HBLADEH/velo-launcher/releases/download/v0.11.0/SHA256SUMS.txt) · [构建身份](https://github.com/HBLADEH/velo-launcher/releases/download/v0.11.0/release-manifest.json)
