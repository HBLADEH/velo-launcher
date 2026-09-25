// Package version 保存运行时可比较的构建版本，供日志、界面与自动更新共用。
package version

const (
	// Number 与发布 tag 对应（去掉前缀 v），是自动更新比较的基准。
	Number = "0.9.2"
	// Repository 是自动更新读取 release 的 GitHub 仓库。
	Repository = "HBLADEH/velo-launcher"
)
