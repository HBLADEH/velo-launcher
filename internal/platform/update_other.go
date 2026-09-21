//go:build !windows

package platform

import "fmt"

// 自动更新依赖 Windows 的安装目录与 PowerShell 脚本，这里只保持包可编译。
func RunDetachedScript(string) error { return fmt.Errorf("当前平台不支持自动更新") }

func InstalledExecutable() (string, bool) { return "", false }
