package app

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"velo-launcher/internal/config"
	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/storage"
)

func TestObsoleteRefreshCannotReplacePublishedIndex(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "index.json")
	original := indexer.Cache{Version: 1, Apps: []model.AppItem{{ID: "original", Name: "Original"}}}
	if err := storage.Write(path, original); err != nil {
		t.Fatal(err)
	}
	s, err := New(dir, slog.New(slog.NewTextHandler(io.Discard, nil)))
	if err != nil {
		t.Fatal(err)
	}
	s.onChange = func() {}
	rev := s.revision
	c := s.Settings()
	c.ScanProgramFiles = !c.ScanProgramFiles
	if err := s.SaveSettings(c); err != nil {
		t.Fatal(err)
	}
	s.state.Scanning = true
	s.commitRefresh(context.Background(), rev, indexer.Cache{Version: 1}, nil, nil, time.Now())
	var disk indexer.Cache
	if err := storage.Read(path, &disk); err != nil {
		t.Fatal(err)
	}
	if len(disk.Apps) != 1 || disk.Apps[0].ID != "original" || len(s.Search("original")) != 1 {
		t.Fatal("obsolete refresh replaced committed index")
	}
	if s.State().Scanning || len(s.wake) != 1 {
		t.Fatal("obsolete refresh did not schedule a replacement")
	}
}

func TestConcurrentSettingsMatchPersistedSnapshot(t *testing.T) {
	s, err := New(t.TempDir(), slog.New(slog.NewTextHandler(io.Discard, nil)))
	if err != nil {
		t.Fatal(err)
	}
	var group sync.WaitGroup
	for n := 0; n < 20; n++ {
		group.Add(1)
		go func(n int) {
			defer group.Done()
			c := s.Settings()
			c.CustomDirectories = []string{filepath.Join(t.TempDir(), fmt.Sprint(n))}
			if err := s.SaveSettings(c); err != nil {
				t.Error(err)
			}
			c.CustomDirectories[0] = "caller mutation"
		}(n)
	}
	group.Wait()
	var disk config.Config
	if err := storage.Read(filepath.Join(s.dir, "config.json"), &disk); err != nil {
		t.Fatal(err)
	}
	if got := s.Settings().CustomDirectories[0]; got != disk.CustomDirectories[0] {
		t.Fatalf("memory %q differs from disk %q", got, disk.CustomDirectories[0])
	}
}

func TestFailedIndexPersistenceDoesNotPruneIcons(t *testing.T) {
	dir := t.TempDir()
	s, err := New(dir, slog.New(slog.NewTextHandler(io.Discard, nil)))
	if err != nil {
		t.Fatal(err)
	}
	s.onChange = func() {}
	iconDir := filepath.Join(dir, "cache", "icons")
	if err := os.MkdirAll(iconDir, 0700); err != nil {
		t.Fatal(err)
	}
	iconPath := filepath.Join(iconDir, "0123456789abcdef0123456789abcdef.png")
	if err := os.WriteFile(iconPath, []byte("retained icon"), 0600); err != nil {
		t.Fatal(err)
	}
	// A directory at the target file forces replacement to fail on Windows.
	if err := os.Mkdir(filepath.Join(dir, "index.json"), 0700); err != nil {
		t.Fatal(err)
	}
	s.commitRefresh(context.Background(), s.revision, indexer.Cache{Version: 1}, nil, nil, time.Now())
	if len(s.State().Warnings) == 0 {
		t.Fatal("persistence failure was not reported")
	}
	if _, err := os.Stat(iconPath); err != nil {
		t.Fatal("failed commit removed persisted index icon", err)
	}
}

func TestCustomDirectoryRefreshIntegration(t *testing.T) {
	if os.Getenv("VELO_INTEGRATION") != "1" {
		t.Skip("set VELO_INTEGRATION=1 for real Windows scanner integration")
	}
	dir, appsDir := t.TempDir(), t.TempDir()
	fixture := filepath.Join(appsDir, "VeloIntegrationFixture.exe")
	if err := os.WriteFile(fixture, []byte("scanner fixture, never executed"), 0600); err != nil {
		t.Fatal(err)
	}
	c := config.Defaults()
	c.ScanProgramFiles = false
	c.CustomDirectories = []string{appsDir}
	if err := storage.Write(filepath.Join(dir, "config.json"), c); err != nil {
		t.Fatal(err)
	}
	s, err := New(dir, slog.New(slog.NewTextHandler(io.Discard, nil)))
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	changes := make(chan struct{}, 2)
	s.Start(ctx, func() {
		select {
		case changes <- struct{}{}:
		default:
		}
	})
	defer s.Close()
	await := func(present bool) {
		t.Helper()
		for {
			select {
			case <-ctx.Done():
				t.Fatal("scanner did not finish", ctx.Err())
			case <-changes:
				if s.State().Scanning {
					continue
				}
				found := false
				for _, item := range s.Search("VeloIntegrationFixture") {
					if item.Path == fixture {
						found = true
					}
				}
				if found != present {
					t.Fatalf("custom directory presence=%v, want %v; state=%+v", found, present, s.State())
				}
				return
			}
		}
	}
	drain := func() {
		for {
			select {
			case <-changes:
			default:
				return
			}
		}
	}
	await(true)
	var cached indexer.Cache
	if err := storage.Read(filepath.Join(dir, "index.json"), &cached); err != nil {
		t.Fatal(err)
	}
	found := false
	for _, item := range cached.Apps {
		if item.Path == fixture {
			found = true
		}
	}
	if !found {
		t.Fatal("custom application not persisted")
	}
	drain()
	c.CustomDirectories = []string{}
	if err := s.SaveSettings(c); err != nil {
		t.Fatal(err)
	}
	await(false)
	drain()
	c.CustomDirectories = []string{appsDir}
	if err := s.SaveSettings(c); err != nil {
		t.Fatal(err)
	}
	await(true)
	drain()
	if err := os.Remove(fixture); err != nil {
		t.Fatal(err)
	}
	s.Refresh()
	await(false)
}

func TestCachedSearchIsMemoryOnly(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "index.json")
	if err := storage.Write(path, indexer.Cache{Version: 1, Apps: []model.AppItem{{ID: "code", Name: "Visual Studio Code"}}}); err != nil {
		t.Fatal(err)
	}
	s, err := New(dir, slog.New(slog.NewTextHandler(io.Discard, nil)))
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	var group sync.WaitGroup
	for n := 0; n < 8; n++ {
		group.Add(1)
		go func() {
			defer group.Done()
			for i := 0; i < 100; i++ {
				if got := s.Search("code"); len(got) != 1 || got[0].ID != "code" {
					t.Error("cached search failed")
				}
			}
		}()
	}
	group.Wait()
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatal("search unexpectedly wrote index")
	}
	c := s.Settings()
	c.Theme = "dark"
	if err := s.SaveSettings(c); err != nil {
		t.Fatal(err)
	}
	if len(s.wake) != 0 {
		t.Fatal("theme change triggered disk scan")
	}
}
func TestCorruptIndexRecovery(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "index.json"), []byte(`{"version":1,"apps":[`), 0600); err != nil {
		t.Fatal(err)
	}
	s, err := New(dir, slog.New(slog.NewTextHandler(io.Discard, nil)))
	if err != nil {
		t.Fatal(err)
	}
	if len(s.Search("")) != 0 || len(s.State().Warnings) == 0 {
		t.Fatal("corrupt index not discarded")
	}
}
