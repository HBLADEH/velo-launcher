package config

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
	"velo-launcher/internal/storage"
)

func TestUpgradeReadsExistingSettingsWithoutRewriting(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.json")
	// Missing newer fields must be filled in memory; explicit opt-outs and
	// fields from another build must survive startup byte for byte on disk.
	original := []byte(`{
  "hotkey": "Ctrl+Alt+K", "max_results": 17, "theme": "dark",
  "launch_at_startup": true, "space_launch": false, "filter_noise": false,
  "search": {"fuzzy": false, "history_weight": 0},
  "scan_program_files": false, "refresh_minutes": 123,
  "auto_check_updates": false, "unknown_option": {"keep": true}
}`)
	if err := os.WriteFile(path, original, 0600); err != nil {
		t.Fatal(err)
	}
	for n := 0; n < 2; n++ {
		c, warning, err := Load(path)
		if err != nil || warning != "" {
			t.Fatalf("existing settings rejected: %q %v", warning, err)
		}
		if c.Hotkey != "Ctrl+Alt+K" || c.MaxResults != 17 || c.Theme != "dark" || !c.LaunchAtStartup || c.SpaceLaunch || c.FilterNoise || c.Search.Fuzzy || c.Search.HistoryWeight != 0 || c.ScanProgramFiles || c.RefreshMinutes != 123 || c.AutoCheckUpdates {
			t.Fatalf("existing choices changed: %+v", c)
		}
		if c.ResultLayout != "list" || c.Version != 1 {
			t.Fatalf("new fields not defaulted: %+v", c)
		}
		data, err := os.ReadFile(path)
		if err != nil || !bytes.Equal(data, original) {
			t.Fatalf("startup rewrote saved configuration: %q %v", data, err)
		}
	}
}

func TestLoadDefaultsAndMigration(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.json")
	c, w, err := Load(path)
	if err != nil || w != "" || c.Hotkey != "Alt+Space" {
		t.Fatalf("%+v %s %v", c, w, err)
	}
	if err := os.WriteFile(path, []byte(`{"max_results":5,"search":{"fuzzy":false}}`), 0600); err != nil {
		t.Fatal(err)
	}
	c, _, err = Load(path)
	if err != nil || c.MaxResults != 5 || c.Search.Fuzzy || c.Search.HistoryWeight != 1 || c.Version != 1 {
		t.Fatalf("migration: %+v %v", c, err)
	}
	// 旧配置缺少启动台开关时沿用默认值：空格启动与辅助项过滤默认开启。
	if !c.SpaceLaunch || !c.FilterNoise || c.ResultLayout != "list" {
		t.Fatalf("defaults not overlayed: %+v", c)
	}
	if err := os.WriteFile(path, []byte(`{"space_launch":false,"filter_noise":false}`), 0600); err != nil {
		t.Fatal(err)
	}
	if c, _, err = Load(path); err != nil || c.SpaceLaunch || c.FilterNoise {
		t.Fatalf("explicit opt-out lost: %+v %v", c, err)
	}
}

func TestResultLayoutRoundTrip(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.json")
	for _, layout := range []string{"grid", "list"} {
		c := Defaults()
		c.ResultLayout = layout
		if err := storage.Write(path, c); err != nil {
			t.Fatal(err)
		}
		got, warning, err := Load(path)
		if err != nil || warning != "" || got.ResultLayout != layout {
			t.Fatalf("layout %q: %+v %q %v", layout, got, warning, err)
		}
	}
	c := Defaults()
	c.ResultLayout = "unknown"
	if c.Validate() == nil {
		t.Fatal("invalid layout accepted")
	}
}
func TestRecoveryAndFutureVersion(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.json")
	if err := os.WriteFile(path, []byte("{broken"), 0600); err != nil {
		t.Fatal(err)
	}
	c, w, err := Load(path)
	if err != nil || w == "" || c.MaxResults != 8 {
		t.Fatal("recovery", err)
	}
	backups, _ := filepath.Glob(path + ".invalid-*")
	if len(backups) != 1 {
		t.Fatal("original config not preserved")
	}
	c.Version = 99
	if err := storage.Write(path, c); err != nil {
		t.Fatal(err)
	}
	if _, _, err = Load(path); err == nil {
		t.Fatal("future schema accepted")
	}
	var saved Config
	if err := storage.Read(path, &saved); err != nil || saved.Version != 99 {
		t.Fatal("future config overwritten")
	}
}
func TestValidation(t *testing.T) {
	for _, mutate := range []func(*Config){func(c *Config) { c.MaxResults = 0 }, func(c *Config) { c.Theme = "no" }, func(c *Config) { c.Search.HistoryWeight = -1 }, func(c *Config) { c.CustomDirectories = []string{"relative"} }, func(c *Config) { c.RefreshMinutes = 0 }} {
		c := Defaults()
		mutate(&c)
		if c.Validate() == nil {
			t.Fatal("invalid settings accepted")
		}
	}
}
