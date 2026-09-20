package config

import (
	"os"
	"path/filepath"
	"testing"
	"velo-launcher/internal/storage"
)

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
	if !c.SpaceLaunch || !c.FilterNoise {
		t.Fatalf("defaults not overlayed: %+v", c)
	}
	if err := os.WriteFile(path, []byte(`{"space_launch":false,"filter_noise":false}`), 0600); err != nil {
		t.Fatal(err)
	}
	if c, _, err = Load(path); err != nil || c.SpaceLaunch || c.FilterNoise {
		t.Fatalf("explicit opt-out lost: %+v %v", c, err)
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
