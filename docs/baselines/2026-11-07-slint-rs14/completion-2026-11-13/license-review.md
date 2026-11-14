# 锁定依赖的许可选项与嵌套声明

2026-11-13。本轮完成两 renderer 锁定 Windows 依赖的技术审查；结果由 `scripts/audit-rust-licenses.ps1` 复现，打包由 `scripts/lib/rust-license-policy.ps1` 执行同一许可选项。旧报告的“有根 LICENSE 文件”和“有补充文本”计数保持原含义。

## 选项与文本

对已锁定的表达式明确选择 MIT、Apache-2.0、BSD、ISC、BSL、Zlib 或 Unicode 的适用项；旧式 `/` 表达式在清单中保留原值并明确选择。`AND` 两侧条件同时保留，不能当作二选一。新表达式没有隐式默认值，会拒绝打包，需先审查。

Slint 的各锁定包选择 **Royalty-free 2.0**。桌面应用保留从设置/关于可访问的真实 `AboutSlint`，许可原文继续随包携带；工程 MIT 不替代依赖许可。原 Go、go-pinyin 数据与 Fluent 图标的来源/文本继续收录。

根文本之外新增递归收集，保持 crate 内相对路径及 SHA，覆盖 `regex-syntax/src/unicode_tables/LICENSE-UNICODE`、`tracing-core/src/spin/LICENSE`、`fontdb/tests/fonts/LICENSE.txt`。最后一项仅为测试字体，字体本体不会因此进入应用包。保守 resolved 清单仍包含 build/optional 节点，不将全部条目称为最终链接依赖。

## Khronos 与 ANGLE

`khronos_api 3.1.0` 将 API registry XML/IDL 作为常量供构建工具使用，许可声明存在文件头。打包额外提取带版权/授权的 `<comment>`、XML 注释或 IDL 开头注释，保存**原始源文件 SHA、源相对路径、提取文本 SHA**；不会仅凭根 Apache 文本覆盖这些声明。

ANGLE XML 还引用独立 BSD 文本。出处沿精确修订追溯：crate 的 `.cargo_vcs_info.json` 指向 gl-rs `f150967b1c44ae888e6676f93f639ebc82771bdc`，`khronos_api` 树为 `87ca063633db8cac918192f3917940eafe6447f2`，`api_angle` submodule 为 `7403dd2cd3764fe96660fe09892e764e9ae1dbca`。从该 ANGLE 修订取得的 LICENSE 原文加入现有补充清单，来源 URL 与 SHA 均固定。没有取最新版许可冒充旧版本。

## 原生与 optional 范围

对带 C/C++/头文件/库或二进制的包逐项检查。`cc`、`cexpr`、`clang-sys`、`skia-bindings` 不在这两份 active Cargo tree 中；Skia 的绑定和其未构建 vendor 不作为本次 exe 的已链接库。`libloading` 的测试 DLL、`webbrowser` 的 iOS 测试头没有进入预览 payload。`windows_x86_64_msvc` 是 Windows import library，保留选定 MIT 条款；系统 DLL 由 Windows 提供，安装器不复制系统 DLL。

该判断绑定实际 `cargo-tree.txt`、metadata 与 exe SHA。将来原 optional 原生包进入 active tree 时，审计会拒绝沿用旧判断，要求重新检查 vendor。公开包仍保存保守清单与全部已收集声明；当前内部预览保留 `RedistributionCleared=false` 和 `PublicReleaseApproved=false`，没有因收齐文本而自动发布或批准源码再分发。
