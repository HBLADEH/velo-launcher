# 启动组合置顶与常用排序

2026-11-25，北京时间。验证使用 Windows 11 x64、锁定工具链和隔离数据目录。两种 Release 预览构建均通过 `./scripts/verify-rust.ps1 -SmokeTest`，各有 140 项 Rust 测试通过、1 项忽略。

- Software SHA-256 为 `066C2BDA5C809538B74386DD322525F59F5EABC3FBD7727529EDC007A005A61B`。
- FemtoVG SHA-256 为 `0CA1F087D3DBEA71FA9594517398E2B59BF33F3B7E5D2E49C0948862904F811B`。

组合回归覆盖初次使用前的首页、两次启动后的常用排序、置顶写入失败、改名与重启、取消置顶、删除和搜索结果状态。工作台回归确认新配置和省略开关字段的配置均默认关闭。

Software 组件专项点击组合列表的星标并核对选中状态。两种渲染器的原生专项各使用三份测试程序，确认两次整组使用累计两次，其中第二次无需再次派发。重试单个已运行成员后，组合历史保持两次。

本机记录位于 `target/launch-group-home-20261125-complete-verify.log` 和 `target/launch-group-home-20261125-*`。 [用户指南](../../user-guide.md#启动组合)
