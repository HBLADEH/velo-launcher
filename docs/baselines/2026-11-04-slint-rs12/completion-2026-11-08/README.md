# RS-12 續驗：自動檢查與冷啟動檢查點

2026-11-08（北京时间）。在当前脏工作区完成两种锁定 Slint renderer 的 Rust 检查、旧版完整回归及 latencyprobe 构建标签测试；随后在独立重启周期尝试 `software-1` 冷启动。自动构建身份见 [verification.json](verification.json)，无效冷尝试见 [cold-software-1.json](cold-software-1.json)。本轮没有新增有效性能或物理输入样本。

## 自动验证

- `./scripts/verify-rust.ps1 -SmokeTest` 退出码 0。FemtoVG 与 Software 各 80 项 Rust 测试通过，格式、clippy、数据双向兼容、RS-10/11 原生检查、Release、窗口/输入/业务/完整页面冒烟通过。
- `GOTOOLCHAIN=go1.25.6 ./scripts/verify.ps1 -SkipBuild` 退出码 0。RS-12 输入及资源观察器回归、26 项前端测试、lint、typecheck/build、`go vet` 和完整 Go 测试通过。
- `GOTOOLCHAIN=go1.25.6 go test -tags latencyprobe ./...` 退出码 0。

PATH 默认 Go 1.23.0 曾在既有 `internal/update/public_test.go` 的 `%2F` HTML fixture 上被 `go vet` 拒绝；按项目记录的 Go 1.25.6 重跑通过，没有修改该测试或产品源码。

本次 Rust Release 产物属于当前验证工作区，不是 RS-12 冻结测量程序。构建时 `WorkspaceDirty=true`；冷启动测量使用原 `.reasonix/rs12-cold-actual-input-20261104/` 冻结副本，没有重建 case。`software-1` 以原 exe 哈希正常退出，但没有有效字符；其余八个原始 case 未尝试。

## 冷启动尝试

用户确认独立 Windows 重启后正式版和候选没有自动运行。启动器记录新的 boot ID 并启动预冻结的 `software-1`。进程退出码为 0，exe SHA-256 与冻结值一致，用户确认从托盘正常退出。测量日志只有一次 `native_key_down`，没有 `input_accepted`；随后出现 `lifecycle_hidden reason=launch`，其代码路径为启动回调。本次没有可用的进程创建到实际字符接收样本，目标应用未写入日志。用户未注意到是否有外部应用打开，并报告现在已关闭；目标仍未知。收集器在写入无效结果后以退出码 1 返回，这是预期的样本拒绝，不是数据未保存。

该启动周期的全局 boot claim 已消耗，不能在本周期尝试别的 case。有效冷样本仍为 **0/9**。沿用原交替顺序，下一项为 `femtovg-1`，需另一次独立重启。由于 `software-1` 无效，原九个 case 全部尝试后仍需额外一个独立 Software case 才可能取得三份有效 Software 样本；不可用本次的失败记录抵数。

## 仍待实测

最新 Windows 启动时间为 `2026-11-07T19:22:23.500000Z`；本周期的 `software-1` 已结束并保留为无效尝试，严格冷样本仍为 **0/9**。下一 case 为 `femtovg-1`；需再次独立执行 Windows“重启”，登录后确认正式版和候选均未自动运行。每个 case 继续使用独立启动周期。

2026-11-07 的三候选微软拼音物理 IME 练习已在该单机 96 DPI 范围通过，沿用[原记录](../completion-2026-11-07/README.md)。Wails 热键稳定性仍未通过：此前四个物理起点只有两个有效字符边界，未达 20 个有效样本；不以本轮合成测试替代。精确首次可输入、输入到结果帧/动画帧校准和呈现时间戳仍待独立录制与校准。默认 renderer 未决，RS-12 保持进行中。
