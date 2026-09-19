package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"velo-launcher/internal/storage"
)

type Search struct {
	Fuzzy         bool    `json:"fuzzy"`
	HistoryWeight float64 `json:"history_weight"`
}
type Config struct {
	Version           int      `json:"version"`
	Hotkey            string   `json:"hotkey"`
	MaxResults        int      `json:"max_results"`
	Theme             string   `json:"theme"`
	LaunchAtStartup   bool     `json:"launch_at_startup"`
	Search            Search   `json:"search"`
	CustomDirectories []string `json:"custom_directories"`
	ScanProgramFiles  bool     `json:"scan_program_files"`
	RefreshMinutes    int      `json:"refresh_minutes"`
}

func Defaults() Config {
	return Config{Version: 1, Hotkey: "Alt+Space", MaxResults: 8, Theme: "system", Search: Search{true, 1}, CustomDirectories: []string{}, ScanProgramFiles: true, RefreshMinutes: 30}
}
func (c Config) Validate() error {
	if c.Version != 1 {
		return fmt.Errorf("unsupported config version %d", c.Version)
	}
	if strings.TrimSpace(c.Hotkey) == "" {
		return fmt.Errorf("快捷键不能为空")
	}
	if c.MaxResults < 1 || c.MaxResults > 20 {
		return fmt.Errorf("结果数量须为 1–20")
	}
	if c.Theme != "system" && c.Theme != "light" && c.Theme != "dark" {
		return fmt.Errorf("无效主题")
	}
	if !(c.Search.HistoryWeight >= 0 && c.Search.HistoryWeight <= 5) {
		return fmt.Errorf("历史权重须为 0–5")
	}
	if c.RefreshMinutes < 1 || c.RefreshMinutes > 1440 {
		return fmt.Errorf("刷新间隔须为 1–1440 分钟")
	}
	for _, dir := range c.CustomDirectories {
		if !filepath.IsAbs(dir) {
			return fmt.Errorf("索引目录须为绝对路径: %s", dir)
		}
	}
	return nil
}

// Load overlays older/partial files onto defaults. Invalid files are preserved
// before recovery. Future schemas are never silently overwritten.
func Load(path string) (Config, string, error) {
	c := Defaults()
	err := storage.Read(path, &c)
	if os.IsNotExist(err) {
		return c, "", storage.Write(path, c)
	}
	if err == nil && c.Version > 1 {
		return c, "", fmt.Errorf("配置来自更新版本，请升级 Velo")
	}
	if err == nil {
		err = c.Validate()
	}
	if err == nil {
		return c, "", nil
	}
	if !os.IsNotExist(err) {
		backup := path + ".invalid-" + time.Now().Format("20060102-150405.000000000")
		if e := os.Rename(path, backup); e != nil {
			return c, "", fmt.Errorf("preserve invalid config: %w", e)
		}
	}
	c = Defaults()
	return c, "配置损坏，已保留原文件并恢复默认设置", storage.Write(path, c)
}
