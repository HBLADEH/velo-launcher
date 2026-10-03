package platform

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"velo-launcher/internal/model"
)

type ItemActions struct {
	OpenDirectory bool `json:"open_directory"`
	RunAsAdmin    bool `json:"run_as_admin"`
}

// Only file targets have an installation directory or support elevation.
// Packaged apps, settings URIs and built-in actions keep their normal launch.
func itemTarget(item model.AppItem) string {
	path := item.ExecPath
	if path == "" {
		path = item.Path
	}
	if !filepath.IsAbs(path) || strings.EqualFold(filepath.Ext(path), ".lnk") {
		return ""
	}
	return filepath.Clean(path)
}

func ActionsFor(item model.AppItem) ItemActions {
	path := itemTarget(item)
	return ItemActions{OpenDirectory: path != "", RunAsAdmin: strings.EqualFold(filepath.Ext(path), ".exe")}
}

func OpenInstallDirectory(item model.AppItem) error {
	path := itemTarget(item)
	if path == "" {
		return fmt.Errorf("此候选项没有可打开的安装目录")
	}
	directory := filepath.Dir(path)
	info, err := os.Stat(directory)
	if err != nil {
		return fmt.Errorf("安装目录已移动或删除: %w", err)
	}
	if !info.IsDir() {
		return fmt.Errorf("安装目录无效")
	}
	return shellExecute(directory, "", "")
}

func LaunchAsAdmin(item model.AppItem) error {
	return launchAsAdmin(item, shellExecuteVerb)
}

func launchAsAdmin(item model.AppItem, execute func(verb, path, args, dir string) error) error {
	path := itemTarget(item)
	if !ActionsFor(item).RunAsAdmin {
		return fmt.Errorf("此候选项不支持使用管理员权限打开")
	}
	info, err := os.Stat(path)
	if err != nil {
		return fmt.Errorf("应用已移动或删除: %w", err)
	}
	if info.IsDir() {
		return fmt.Errorf("应用路径指向目录")
	}
	// Resolve .lnk targets but retain profile arguments and working directory.
	return execute("runas", path, item.Arguments, item.WorkingDirectory)
}
