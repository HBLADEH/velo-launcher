//go:build !windows

package platform

import "fmt"

func OpenDirectory(string) error { return fmt.Errorf("当前平台不支持打开目录") }
