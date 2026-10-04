package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// Backup copies the saved file verbatim, including fields unknown to this
// version. It never rewrites the source or replaces a previous backup.
func Backup(path, directory string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", fmt.Errorf("读取当前配置失败: %w", err)
	}
	directory, err = filepath.Abs(directory)
	if err != nil {
		return "", err
	}
	if err := os.MkdirAll(directory, 0700); err != nil {
		return "", fmt.Errorf("创建备份目录失败: %w", err)
	}
	file, err := os.CreateTemp(directory, ".config-*.tmp")
	if err != nil {
		return "", fmt.Errorf("创建配置备份失败: %w", err)
	}
	defer os.Remove(file.Name())
	if _, err := file.Write(data); err != nil {
		file.Close()
		return "", fmt.Errorf("写入配置备份失败: %w", err)
	}
	if err := file.Sync(); err != nil {
		file.Close()
		return "", fmt.Errorf("写入配置备份失败: %w", err)
	}
	if err := file.Close(); err != nil {
		return "", err
	}
	// Keep the random suffix as well as the timestamp so rapid backups are unique.
	name := strings.TrimSuffix(strings.TrimPrefix(filepath.Base(file.Name()), ".config-"), ".tmp")
	destination := filepath.Join(directory, "config-"+time.Now().Format("20060102-150405")+"-"+name+".json")
	if err := os.Rename(file.Name(), destination); err != nil {
		return "", fmt.Errorf("保存配置备份失败: %w", err)
	}
	return destination, nil
}
