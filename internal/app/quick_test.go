package app

import (
	"context"
	"fmt"
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

func TestHomeLearnsUsageBeforeColdStartAndPersists(t *testing.T) {
	dir := t.TempDir()
	items := []model.AppItem{{ID: "used", Name: "Office Forensic", Source: "Program Files"}, {ID: "once", Name: "Once", Source: "Desktop"}}
	for n := 0; n < 15; n++ {
		items = append(items, model.AppItem{ID: fmt.Sprint(n), Name: fmt.Sprintf("Chrome %d", n), Source: "Desktop"})
	}
	if err := storage.Write(filepath.Join(dir, "index.json"), indexer.Cache{Version: 1, Apps: items}); err != nil {
		t.Fatal(err)
	}
	s := quickService(t, dir)
	if err := s.Record("once", ""); err != nil {
		t.Fatal(err)
	}
	for n := 0; n < 5; n++ {
		if err := s.Record("used", "forensic"); err != nil {
			t.Fatal(err)
		}
	}
	for _, current := range []*Service{s, quickService(t, dir)} {
		current.config.Search.HistoryWeight = 0
		got := current.Home().Frequent
		if len(got) != 12 || got[0].ID != "used" || got[1].ID != "once" {
			t.Fatalf("personal history hidden by defaults: %+v", got)
		}
		if err := current.SetPinned("used", true); err != nil {
			t.Fatal(err)
		}
		if current.Home().Frequent[0].ID != "once" {
			t.Fatal("pinned app duplicated in frequent")
		}
	}
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
	if len(home.Pinned) != 2 || len(home.Frequent) != 0 || len(home.Tools) != len(s.tools)-1 {
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

func TestSystemToolsSearchWithoutAppCandidates(t *testing.T) {
	s := quickService(t, t.TempDir())
	s.config.MaxResults = 100 // Inspect all matches, including intentionally shared aliases.
	s.config.Search.Fuzzy = false
	if s.State().Count != 0 || len(s.Home().Pinned) != 0 {
		t.Fatal("expected no app candidates or pins")
	}
	for _, tool := range systemTools() {
		t.Run(tool.ID, func(t *testing.T) {
			for _, query := range append([]string{tool.Name}, tool.Keywords...) {
				results := s.Search(query)
				found := false
				for _, result := range results {
					if result.ID == tool.ID {
						found = true
						if result.Pinned {
							t.Fatal("unfixed system entry reported as pinned")
						}
						item, err := s.Item(result.ID)
						if err != nil || item.ExecPath != tool.ExecPath || item.Arguments != tool.Arguments {
							t.Fatalf("search result lost launch target: %+v, %v", item, err)
						}
					}
				}
				if !found {
					t.Errorf("query %q missing %s: %+v", query, tool.ID, results)
				}
			}
			if err := s.SetPinned(tool.ID, true); err != nil {
				t.Fatal(err)
			}
			if err := s.SetPinned(tool.ID, false); err != nil {
				t.Fatal(err)
			}
			results := s.Search(tool.Name)
			if len(results) == 0 || results[0].ID != tool.ID || results[0].Pinned {
				t.Fatalf("unpin removed system entry from search: %+v", results)
			}
		})
	}
}

func TestUnavailableSystemPinIsHiddenAndReturnsWhenAvailable(t *testing.T) {
	s := quickService(t, t.TempDir())
	original := append([]model.AppItem{}, s.tools...)
	if err := s.SetPinned("system:environment", true); err != nil {
		t.Fatal(err)
	}
	s.tools = nil
	s.replace(nil)
	if len(s.Home().Pinned) != 0 || len(s.Search("环境变量")) != 0 {
		t.Fatal("unavailable system tool retained in UI")
	}
	if _, err := s.Item("system:environment"); err == nil {
		t.Fatal("unavailable system tool still launchable")
	}
	s.tools = original
	s.replace(nil)
	if len(s.Home().Pinned) != 1 {
		t.Fatal("temporary absence discarded saved pin")
	}
	if err := s.Record("system:devices", "sbglq"); err != nil {
		t.Fatal(err)
	}
	if s.Home().Tools[0].ID != "system:devices" {
		t.Fatal("system tools did not learn usage")
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
