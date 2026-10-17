# 实现原理配图 · Implementation illustration

`velo-how-it-works.png` 用卡通角色介绍 Velo 的主要实现流程：Vue 界面通过 Wails 调用 Go，Go 组织应用目录、内存搜索与历史，再由 Windows 平台层执行启动。它是概念插画，实际界面截图见 [截图说明](../images/README.md)。

中英文 README 共用此图，并在正文提供角色、流程和代码入口。实现细节以源码为准；修改扫描、搜索或启动流程时，请同步检查配图和两份 README。

The READMEs share this conceptual illustration. Vue forwards user actions through Wails to Go, which manages the local catalog, search, and history, and delegates launch requests to Windows. The surrounding text explains each step in both languages. Check the illustration and both READMEs when changing these flows.

## 生成记录

- 日期：2026-10-18
- 工具：内置 `imagegen`
- 输入：文字提示词，无参考图
- 提示词：见 [velo-how-it-works.prompt.txt](velo-how-it-works.prompt.txt)
