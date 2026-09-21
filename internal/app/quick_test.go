package app

import (
	"context"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"testing"
	"time"

	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/storage"
)

func quickService(t *testing.T, dir string) *Service {
	t.Helper()
	s, err := New(dir, slog.New(slog.NewTextHandler(io.Discard, nil)))
	if err != nil {
		t.Fatal(err)
	}
	return s
}

func TestImportPersistsDeduplicatesAndSurvivesRefresh(t *testing.T) {
	dir := t.TempDir()
	s := quickService(t, dir)
	path := filepath.Join(t.TempDir(), "PersonalUpdater.exe")
	// A scanner fixture, not a runnable program; importing must not launch it.
	if err := os.WriteFile(path, []byte("fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	result, err := s.AddApplications([]string{path, path, filepath.Join(dir, "notes.txt"), filepath.Join(dir, "missing.exe")})
	if err != nil || result.Added != 1 || len(result.Warnings) != 2 {
		t.Fatalf("import = %+v, %v", result, err)
	}
	s = quickService(t, dir)
	if len(s.CustomApplications()) != 1 || len(s.Home().Pinned) != 0 {
		t.Fatal("import not restored")
	}
	id := s.CustomApplications()[0].ID
	s.commitRefresh(context.Background(), s.revision, indexer.Cache{Version: 1}, nil, nil, time.Now())
	if got := s.Search("PersonalUpdater"); len(got) != 1 || got[0].Pinned {
		t.Fatalf("manual entry lost to refresh/filter: %+v", got)
	}
	if err := s.SetPinned(id, true); err != nil {
		t.Fatal(err)
	}
	if err := s.SetPinned(id, false); err != nil {
		t.Fatal(err)
	}
	s = quickService(t, dir)
	if got := s.Search("PersonalUpdater"); len(got) != 1 || got[0].Pinned {
		t.Fatal("unpin removed custom search entry")
	}
	if err := s.SetPinned(id, true); err != nil {
		t.Fatal(err)
	}
	if err := s.DeleteApplication(id); err != nil {
		t.Fatal(err)
	}
	if len(s.Home().Pinned) != 0 || len(s.Search("PersonalUpdater")) != 0 {
		t.Fatal("manual entry not removed")
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatal("removing shortcut deleted original file", err)
	}
	s = quickService(t, dir)
	if len(s.Home().Pinned) != 0 || len(s.CustomApplications()) != 0 || len(s.Search("PersonalUpdater")) != 0 {
		t.Fatal("removal not persisted")
	}
}

func TestPinIndexedAppAndSystemTool(t *testing.T) {
	dir := t.TempDir()
	item := model.AppItem{ID: "editor", Name: "Test Editor", Source: "Start Menu"}
	if err := storage.Write(filepath.Join(dir, "index.json"), indexer.Cache{Version: 1, Apps: []model.AppItem{item}}); err != nil {
		t.Fatal(err)
	}
	s := quickService(t, dir)
	for _, id := range []string{"editor", "editor", "system:environment"} {
		if err := s.SetPinned(id, true); err != nil {
			t.Fatal(err)
		}
	}
	home := s.Home()
	if len(home.Pinned) != 2 || len(home.Frequent) != 0 || len(home.Tools) != 7 {
		t.Fatalf("home groups not disjoint: %+v", home)
	}
	if got := s.Search("环境变量"); len(got) != 1 || !got[0].Pinned {
		t.Fatalf("system search: %+v", got)
	}
	if err := s.SetPinned("editor", false); err != nil {
		t.Fatal(err)
	}
	if got := s.Search("Test Editor"); len(got) != 1 || got[0].Pinned {
		t.Fatal("unpin removed indexed application")
	}
	if _, err := s.Item("system:environment"); err != nil {
		t.Fatal("system tool cannot be launched", err)
	}
}

func TestQuickPersistenceFailureDoesNotPublish(t *testing.T) {
	dir := t.TempDir()
	s := quickService(t, dir)
	if err := os.Mkdir(filepath.Join(dir, "application-library.json"), 0700); err != nil {
		t.Fatal(err)
	}
	if err := s.SetPinned("system:calculator", true); err == nil {
		t.Fatal("expected write failure")
	}
	if len(s.Home().Pinned) != 0 {
		t.Fatal("failed write changed memory")
	}
}

func TestRefreshRetainsManualIcon(t *testing.T) {
	dir := t.TempDir()
	name := "0123456789abcdef0123456789abcdef.png"
	iconDir := filepath.Join(dir, "cache", "icons")
	if err := os.MkdirAll(iconDir, 0700); err != nil {
		t.Fatal(err)
	}
	iconPath := filepath.Join(iconDir, name)
	if err := os.WriteFile(iconPath, []byte("retained icon fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := storage.Write(filepath.Join(dir, "quick-launch.json"), []model.AppItem{{ID: "manual", Name: "Manual", Source: "Quick Launch", IconURL: "/icons/" + name}}); err != nil {
		t.Fatal(err)
	}
	s := quickService(t, dir)
	s.commitRefresh(context.Background(), s.revision, indexer.Cache{Version: 1}, nil, nil, time.Now())
	if _, err := os.Stat(iconPath); err != nil {
		t.Fatal("refresh removed manual application's icon", err)
	}
}

type quickResolver struct{}

func (quickResolver) Resolve(string) (model.AppItem, error) {
	return model.AppItem{ExecPath: `C:\Apps\Editor.exe`, Arguments: "--profile work", WorkingDirectory: `C:\Projects`, IconPath: "editor.ico"}, nil
}

func TestQuickShortcutRetainsLaunchSemantics(t *testing.T) {
	path := filepath.Join(t.TempDir(), "Editor.lnk")
	if err := os.WriteFile(path, []byte("fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	item, err := quickItem(path, quickResolver{})
	if err != nil {
		t.Fatal(err)
	}
	if item.Path != path || item.Arguments != "--profile work" || item.WorkingDirectory != `C:\Projects` || item.ID != model.Identity(item.ExecPath, item.Arguments) {
		t.Fatalf("shortcut metadata changed: %+v", item)
	}
	if _, err := quickItem("relative.exe", quickResolver{}); err == nil {
		t.Fatal("accepted relative path")
	}
	directory := filepath.Join(t.TempDir(), "directory.exe")
	if err := os.Mkdir(directory, 0700); err != nil {
		t.Fatal(err)
	}
	if _, err := quickItem(directory, quickResolver{}); err == nil {
		t.Fatal("accepted directory")
	}
}
