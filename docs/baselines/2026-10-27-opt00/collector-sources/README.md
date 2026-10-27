# 当次采集源码快照

公开说明：本页关联的公开结果已脱敏；原始日志与源码快照仅本地保留，见[公开范围与校验口径](../../README.md)。

这些文件用于追溯 2026-10-27 三份资源报告的实际采集代码。执行时尚未提交改动，因此以 SHA-256 对应源码快照，保留原始报告中的版本。后续测量使用项目 `scripts/` 中已补齐 UTF-8 兼容性的版本。

| 文件 | 对应报告 | SHA-256 |
| --- | --- | --- |
| measure-resources-before-input-fingerprints.ps1（本地原件：`measure-resources-before-input-fingerprints.ps1`，不入库） | 可见 / 持续隐藏观察 ≥30 秒 | `27ac08ed4857ed9b250b2410da00f4265d74a5430d84ee91516edc4384c17787` |
| measure-resources.ps1（本地原件：`measure-resources.ps1`，不入库） | 持续隐藏观察 ≥5 分钟 | `56e200651edf7581d72c5ad15a925ecd83d8fe4c823f3e29b47fe6958226fbf0` |
| lib/resource-sample.ps1（本地原件：`lib/resource-sample.ps1`，不入库） | 三份报告共用的判定代码 | `eefdf2ec5d89ffbaf2084accbfa078e2dc4bda05e4b10fec5c849be065261476` |

早期采集器从最终未修复编码的采集器移除“逐样本输入指纹”函数和两行字段写入还原，SHA-256 与早期原始报告完全匹配；未改写任何历史采样数据。两版都保存整组输入指纹。原始报告全部由 PowerShell 7 采集；源码快照不是后续 PowerShell 5.1 的推荐入口。
