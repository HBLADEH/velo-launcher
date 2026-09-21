package main

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	wruntime "github.com/wailsapp/wails/v2/pkg/runtime"
	"velo-launcher/internal/platform"
	"velo-launcher/internal/update"
	"velo-launcher/internal/version"
)

// autoCheckDelay 让启动时的索引扫描先完成，避免同时占用网络与磁盘。
const autoCheckDelay = 12 * time.Second

func currentVersion() (update.Version, error) { return update.ParseVersion(version.Number) }

// CheckUpdate 供设置面板手动检查，也用于安装前重新确认最新版本。
func (a *App) CheckUpdate() (update.Info, error) {
	current, err := currentVersion()
	if err != nil {
		return update.Info{}, err
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	return a.updates.Check(ctx, current)
}

// InstallUpdate 下载、校验并安装新版本，然后退出 Velo 让脚本完成替换。
// 便携版直接覆盖当前可执行文件；安装目录不可写时改用安装包静默升级。
func (a *App) InstallUpdate() error {
	current, err := currentVersion()
	if err != nil {
		return err
	}
	// 下载 11 MB 左右的程序在慢速网络下可能持续数分钟。
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Minute)
	defer cancel()
	info, err := a.updates.Check(ctx, current)
	if err != nil {
		return err
	}
	if !info.Available {
		return fmt.Errorf("当前已是最新版本 %s", current.String())
	}
	if info.Checksum.URL == "" {
		return fmt.Errorf("v%s 未提供 SHA-256 校验文件，已停止自动安装，请前往发布页手动下载", info.Version)
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	portable := platform.DirectoryWritable(filepath.Dir(exe))
	asset := info.Portable
	if !portable {
		if asset = info.Installer; asset.URL == "" {
			return fmt.Errorf("当前安装目录不可写，且 v%s 未提供安装包，请前往发布页手动下载", info.Version)
		}
	}
	if asset.URL == "" {
		return fmt.Errorf("v%s 未提供可用的下载文件", info.Version)
	}
	sums, err := a.updates.Checksums(ctx, info.Checksum.URL)
	if err != nil {
		return err
	}
	expected, ok := sums[asset.Name]
	if !ok {
		return fmt.Errorf("校验文件缺少 %s 的记录，已停止自动安装", asset.Name)
	}
	dir := filepath.Join(a.service.DataDir(), "updates")
	dest := filepath.Join(dir, asset.Name)
	sum, err := a.updates.Download(ctx, asset.URL, dest, a.reportProgress)
	if err != nil {
		return err
	}
	if !strings.EqualFold(sum, expected) {
		os.Remove(dest)
		return fmt.Errorf("更新文件的 SHA-256 与发布记录不一致，已删除下载内容，请重试")
	}
	target, script := exe, platform.ReplaceScript(exe, dest)
	if !portable {
		// 安装版可能被装到别处，优先回到注册表记录的安装目录。
		if installed, ok := platform.InstalledExecutable(); ok {
			target = installed
		}
		script = platform.InstallScript(target, dest)
	}
	scriptPath := filepath.Join(dir, fmt.Sprintf("velo-update-%d.ps1", os.Getpid()))
	if err := platform.WriteScript(scriptPath, script); err != nil {
		return err
	}
	if err := platform.RunDetachedScript(scriptPath); err != nil {
		return err
	}
	a.logger.Info("update scheduled", "version", info.Version, "portable", portable, "target", target)
	// 先让绑定调用返回，脚本需要 Velo 退出后才能覆盖可执行文件。
	go func() {
		time.Sleep(time.Second)
		a.Quit()
	}()
	return nil
}

func (a *App) reportProgress(done, total int64) {
	if total <= 0 || a.ctx == nil {
		return
	}
	wruntime.EventsEmit(a.ctx, "update:progress", int(done*100/total))
}

// autoCheckUpdate 在启动后检查一次新版本，并只发出事件让界面提示。
// 下载与安装始终由用户确认，不会在后台静默替换程序。
func (a *App) autoCheckUpdate(ctx context.Context) {
	if !a.service.Settings().AutoCheckUpdates {
		return
	}
	select {
	case <-ctx.Done():
		return
	case <-time.After(autoCheckDelay):
	}
	current, err := currentVersion()
	if err != nil {
		a.logger.Warn("update check skipped", "error", err)
		return
	}
	checkCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	info, err := a.updates.Check(checkCtx, current)
	if err != nil {
		a.logger.Warn("update check failed", "error", err)
		return
	}
	if !info.Available {
		return
	}
	a.logger.Info("update available", "version", info.Version)
	wruntime.EventsEmit(ctx, "update:available", info)
}
