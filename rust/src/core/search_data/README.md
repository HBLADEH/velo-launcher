# 搜索兼容静态表

来源为仓库锁定的 `github.com/mozillazg/go-pinyin v0.21.0` 和 Go 1.25.6 的 Unicode 15.0.0 表。`cmd/rs09-tables` 从真实 API 生成；不复制一份新的拼音转换算法，也不在运行时依赖 Go。

- `lower.bin`：Go `unicode.ToLower` 的非恒等映射。
- `word.bin`：`unicode.IsLetter || unicode.IsDigit` 的闭区间；不将所有数字类符号当作数字。
- `han.bin`：`unicode.Han` 的闭区间。
- `pinyin.bin`：按码点排序的 41,923 项，Normal / Heteronym=false 的第一个读音。包括词典内非 Han 字符，用于混合名称中的转换。
- `syllables.bin`：共享 UTF-8 读音，以 NUL 结束。

前四个文件均为连续小端 `(u32, u32)`；拼音第二列是 syllables 的字节偏移。总静态表 354,410 字节；直接嵌入只读数据，以二分查找读取，不创建全量 HashMap。仅在构建内存索引时生成拼音后缀和首字母，查询不转换拼音。该大小不是进程内存测量。

在仓库根目录使用 Go 1.25.6：

```powershell
go run ./cmd/rs09-tables check
# 只有有意更新基线时才重新生成，随后审查字节差异并重跑兼容测试：
go run ./cmd/rs09-tables write
```

词典的 MIT 许可见 [go-pinyin-MIT.txt](../../../licenses/go-pinyin-MIT.txt)，Unicode 表生成依据的 Go BSD 许可见 [Go-BSD.txt](../../../licenses/Go-BSD.txt)。原始词典：[go-pinyin v0.21.0](https://github.com/mozillazg/go-pinyin/tree/v0.21.0)；使用仓库 `go.sum` 校验的模块缓存，不访问用户目录内容。最终发行 notices 继续按 RS-14 汇总。
