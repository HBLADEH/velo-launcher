package app

import (
	"context"
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
