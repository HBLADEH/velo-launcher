package main

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	core "velo-launcher/internal/app"
	"velo-launcher/internal/update"
)

func backupTestApp(t *testing.T) (*App, []byte) {
	t.Helper()
	dir := t.TempDir()
	original := []byte("{\r\n  \"hotkey\": \"Ctrl+Alt+K\", \"theme\": \"dark\", \"space_launch\": false\r\n}\r\n")
	if err := os.WriteFile(filepath.Join(dir, "config.json"), original, 0600); err != nil {
		t.Fatal(err)
	}
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	service, err := core.New(dir, logger)
	if err != nil {
		t.Fatal(err)
	}
	return NewApp(logger, service, false, false, time.Now()), original
}

func TestManualAndUpdateBackupsPreserveConfiguration(t *testing.T) {
	for _, portable := range []bool{true, false} {
		t.Run(fmt.Sprintf("portable=%t", portable), func(t *testing.T) {
			app, original := backupTestApp(t)
			manual, err := app.BackupSettings()
			if err != nil {
				t.Fatal(err)
			}
			dir := app.service.DataDir()
			updateDir := filepath.Join(dir, "updates")
			if err := os.MkdirAll(updateDir, 0700); err != nil {
				t.Fatal(err)
			}
			script, err := app.prepareUpdate(filepath.Join(dir, "velo.exe"), filepath.Join(updateDir, "download.exe"), portable)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := os.Stat(script); err != nil {
				t.Fatal(err)
			}
			backups, err := filepath.Glob(filepath.Join(app.service.BackupDir(), "*.json"))
			if err != nil || len(backups) != 2 {
				t.Fatalf("automatic backup missing: %v %v", backups, err)
			}
			for _, path := range append(backups, manual, filepath.Join(dir, "config.json")) {
				data, err := os.ReadFile(path)
				if err != nil || !bytes.Equal(data, original) {
					t.Fatalf("configuration changed at %s: %q %v", path, data, err)
				}
			}
			// Opening the next version must reuse the user's saved choices.
			restarted, err := core.New(dir, app.logger)
			if err != nil {
				t.Fatal(err)
			}
			if c := restarted.Settings(); c.Hotkey != "Ctrl+Alt+K" || c.Theme != "dark" || c.SpaceLaunch {
				t.Fatalf("restart reset configuration: %+v", c)
			}
		})
	}
}

func TestInstallUpdateStopsWhenConfigurationBackupFails(t *testing.T) {
	app, original := backupTestApp(t)
	if err := os.WriteFile(app.service.BackupDir(), []byte("blocked"), 0600); err != nil {
		t.Fatal(err)
	}
	const payload = "test update payload"
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/SHA256SUMS.txt":
			fmt.Fprintf(w, "%x  velo-launcher.exe\n", sha256.Sum256([]byte(payload)))
		case "/velo-launcher.exe":
			fmt.Fprint(w, payload)
		default:
			fmt.Fprintf(w, `[{"tag_name":"v9.9.9","assets":[{"name":"velo-launcher.exe","browser_download_url":"http://%s/velo-launcher.exe"},{"name":"SHA256SUMS.txt","browser_download_url":"http://%s/SHA256SUMS.txt"}]}]`, r.Host, r.Host)
		}
	}))
	defer server.Close()
	app.updates = &update.Client{Repository: "acme/velo", BaseURL: server.URL, HTTP: server.Client()}
	if err := app.InstallUpdate(); err == nil || !strings.Contains(err.Error(), "更新前备份配置失败，已停止安装") {
		t.Fatalf("backup failure did not stop installation: %v", err)
	}
	scripts, err := filepath.Glob(filepath.Join(app.service.DataDir(), "updates", "*.ps1"))
	if err != nil || len(scripts) != 0 {
		t.Fatalf("failed backup left an executable update script: %v %v", scripts, err)
	}
	data, err := os.ReadFile(filepath.Join(app.service.DataDir(), "config.json"))
	if err != nil || !bytes.Equal(data, original) {
		t.Fatalf("failed update changed configuration: %q %v", data, err)
	}
	if app.exitRequested {
		t.Fatal("failed backup exited Velo")
	}
}
