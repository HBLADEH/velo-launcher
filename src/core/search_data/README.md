# 搜索静态数据

原始来源为 `github.com/mozillazg/go-pinyin v0.21.0` 与 Go 1.25.6 的 Unicode 15.0.0 数据，由迁移期 `cmd/rs09-tables` 生成。五份二进制共 354,410 字节，直接嵌入 Rust 只读数据；构建、查询和运行均不需要 Go。

- `lower.bin`：Unicode 简单小写的非恒等映射。
- `word.bin`：字母或十进制数字的闭区间。
- `han.bin`：Han 字符的闭区间。
- `pinyin.bin`：按码点排序的 41,923 项及读音偏移，取基线词典首个读音。
- `syllables.bin`：以 NUL 结束的共享 UTF-8 读音。

前四份是连续小端 `(u32, u32)`。读取采用二分查找，不建立全量 HashMap；只在构建应用索引时转换拼音。

`tests/fixtures/compat/search-tables.json` 固化完整 SHA-256，Rust 契约测试核对全部字节，并用搜索用例检查排序。源生成器保存在整理前提交 `cc65619`，在独立检出中才复现历史生成；有意更新数据时需审查来源、许可、字节变化和兼容用例。

词典来源 [go-pinyin v0.21.0](https://github.com/mozillazg/go-pinyin/tree/v0.21.0)，保留 [MIT](../../../licenses/go-pinyin-MIT.txt) 与 [Go BSD](../../../licenses/Go-BSD.txt) 许可。
