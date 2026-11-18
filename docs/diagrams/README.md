# README 实现流程图

[返回项目首页](../../README.md) · [Rust 工程说明](../rust-engineering.md)

- [中文版](velo-how-it-works.svg)用于中文 README。
- [英文版](velo-how-it-works.en.svg)用于英文 README。

流程图依据当前根目录的 Rust + Slint 工程绘制：单进程内分为界面线程、内存搜索和有界后台任务，Windows 系统调用集中在平台层。Everything 以虚线标记为默认关闭的可选外部进程；用户数据在本机保存。图示不表示新的测试或性能结论。

图形使用 SVG，文字可搜索，包含无障碍标题与说明，随窗口缩放保持清晰。源码是 [generate-readme-diagrams.py](../../scripts/generate-readme-diagrams.py)，仅使用 Python 标准库；不参与应用构建。

在仓库根目录使用 Python 3.10+ 重新生成或检查：

```powershell
python ./scripts/generate-readme-diagrams.py
python ./scripts/generate-readme-diagrams.py --check
```

修改时同步核对 `src/main.rs`、界面适配、`src/core/service.rs`、搜索、存储和 Windows 平台层的实际调用关系。重新生成后检查中英文图的文字、连线、箭头和 README 显示宽度。不要把尚未实现的功能画成现有链路。
