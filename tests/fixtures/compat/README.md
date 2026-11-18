# 固定兼容用例

此目录保留迁移期从真实 Go 基线固化的合成 JSON，不含用户数据或个人路径。主线 Rust 测试只读取预期，不再运行 Go 或自动刷新预期。

- `rs08.json`：应用 ID、默认配置、版本/范围校验、历史/应用库/索引结构、一次迁移与修改后预期。
- `rs09.json`：Unicode 归一化、有序搜索、历史学习、固定与首页，包含中文、拼音、别名、多条件及稳定排序。
- `rs10.json`：名称过滤、来源优先级、去重、发现与动作能力。
- `rs11.json` / `rs11.expected.json`：版本比较、Windows 系统快捷与协议动作规则。
- `search-tables.json`：迁移期五份搜索静态数据的完整 SHA-256；文件字节与原始预期保留不变。

```powershell
./scripts/verify-rust.ps1 -SmokeTest
# 需要时单独执行
./scripts/test-data-compat.ps1 -Renderer Software
./scripts/test-search-compat.ps1 -Renderer Software
./scripts/test-discovery-compat.ps1 -Renderer Software
```

数据检查使用全新临时目录，覆盖写回、未知字段、原文不重写、只读/拒绝写入、原子替换失败和事务回滚；失败时原文与内存快照保持一致。完整入口额外检查真实 Shell 快捷方式、增量重用、图标、后台启动与重启，界面冒烟用合成组件输入。

旧 Go 产品位于 [go 分支](https://github.com/HBLADEH/velo-launcher/tree/go)。迁移期 oracle 和生成命令的最后副本在整理前提交 `cc65619`，可在独立检出中复现。新行为有意变更时同时审查预期差异；不要将 Rust 自己生成的结果直接当作独立基线。
