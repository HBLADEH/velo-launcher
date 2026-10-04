package config

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestBackupPreservesSavedBytesAndEarlierBackups(t *testing.T) {
	dir := t.TempDir()
	source := filepath.Join(dir, "config.json")
	directory := filepath.Join(dir, "备份")
	original := []byte("{\r\n  \"theme\": \"dark\", \"unknown_option\": \"保留\"\r\n}\r\n")
	if err := os.WriteFile(source, original, 0600); err != nil {
		t.Fatal(err)
	}
	seen := map[string]bool{}
	for n := 0; n < 10; n++ {
		path, err := Backup(source, directory)
		if err != nil {
			t.Fatal(err)
		}
		if seen[path] || !filepath.IsAbs(path) || filepath.Dir(path) != directory || filepath.Ext(path) != ".json" {
			t.Fatalf("invalid or reused backup path: %q", path)
		}
		seen[path] = true
	}
	for path := range seen {
		data, err := os.ReadFile(path)
		if err != nil || !bytes.Equal(data, original) {
			t.Fatalf("backup changed saved bytes: %q %v", data, err)
		}
	}
	data, err := os.ReadFile(source)
	if err != nil || !bytes.Equal(data, original) {
		t.Fatalf("backup modified source: %q %v", data, err)
	}
	entries, err := os.ReadDir(directory)
	if err != nil || len(entries) != len(seen) {
		t.Fatalf("backup lost files or left temporary files: %v %v", entries, err)
	}
}

func TestBackupFailuresLeaveSourceUntouched(t *testing.T) {
	dir := t.TempDir()
	source := filepath.Join(dir, "config.json")
	directory := filepath.Join(dir, "backups")
	if path, err := Backup(source, directory); err == nil || path != "" {
		t.Fatalf("missing config backed up: %q %v", path, err)
	}
	if _, err := os.Stat(directory); !os.IsNotExist(err) {
		t.Fatalf("failed source read created backups: %v", err)
	}
	original := []byte(`{"hotkey":"Ctrl+Space"}`)
	if err := os.WriteFile(source, original, 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(directory, []byte("blocked"), 0600); err != nil {
		t.Fatal(err)
	}
	if path, err := Backup(source, directory); err == nil || path != "" {
		t.Fatalf("backup to file accepted: %q %v", path, err)
	}
	data, err := os.ReadFile(source)
	if err != nil || !bytes.Equal(data, original) {
		t.Fatalf("failed backup modified source: %q %v", data, err)
	}
}
