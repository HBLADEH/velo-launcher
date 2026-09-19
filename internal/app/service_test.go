package app

import (
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"testing"

	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/storage"
)

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
