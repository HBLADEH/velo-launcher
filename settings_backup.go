package main

import (
	"fmt"
	"os"

	"velo-launcher/internal/platform"
)

func (a *App) BackupSettings() (string, error) {
	return a.service.BackupSettings()
}

func (a *App) OpenBackupDirectory() error {
	directory := a.service.BackupDir()
	if err := os.MkdirAll(directory, 0700); err != nil {
		return fmt.Errorf("创建备份目录失败: %w", err)
	}
	return platform.OpenDirectory(directory)
}
