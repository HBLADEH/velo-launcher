package app

import (
	"os"
	"path/filepath"
	"testing"

	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/storage"
)

func TestLegacyImportMigrationDoesNotResurrectDeletedApp(t *testing.T) {
	dir := t.TempDir()
	legacy := []model.AppItem{{ID: "manual", Name: "Outside App", Source: "Quick Launch"}, {ID: "scanned", Name: "Scanned", Source: "Start Menu"}}
	if err := storage.Write(filepath.Join(dir, "quick-launch.json"), legacy); err != nil {
		t.Fatal(err)
	}
	s := quickService(t, dir)
	if len(s.CustomApplications()) != 1 || s.CustomApplications()[0].Source != "Manual" || len(s.Home().Pinned) != 2 {
		t.Fatal("legacy entries not migrated correctly")
	}
	if err := s.SetPinned("manual", false); err != nil {
		t.Fatal(err)
	}
	s = quickService(t, dir)
	if got := s.Search("Outside App"); len(got) != 1 || got[0].Pinned {
		t.Fatal("unpin lost migrated entry")
	}
	if err := s.DeleteApplication("manual"); err != nil {
		t.Fatal(err)
	}
	s = quickService(t, dir)
	if len(s.CustomApplications()) != 0 || len(s.Search("Outside App")) != 0 {
		t.Fatal("legacy backup resurrected deleted entry")
	}
	if _, err := os.Stat(filepath.Join(dir, "quick-launch.json")); err != nil {
		t.Fatal("migration removed original backup")
	}
}

func TestDeleteApplicationFailureKeepsSearchAndPin(t *testing.T) {
	dir := t.TempDir()
	item := model.AppItem{ID: "custom", Name: "Outside App", Source: "Manual"}
	path := filepath.Join(dir, "application-library.json")
	if err := storage.Write(path, applicationLibrary{1, []model.AppItem{item}, []model.AppItem{item}}); err != nil {
		t.Fatal(err)
	}
	s := quickService(t, dir)
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(path, 0700); err != nil {
		t.Fatal(err)
	}
	if err := s.DeleteApplication(item.ID); err == nil {
		t.Fatal("expected save failure")
	}
	if len(s.CustomApplications()) != 1 || len(s.Home().Pinned) != 1 || len(s.Search("Outside App")) != 1 {
		t.Fatal("failed removal changed memory")
	}
}

func TestCustomAndScannedApplicationShareOneSearchResult(t *testing.T) {
	dir := t.TempDir()
	item := model.AppItem{ID: "same", Name: "Same App", Source: "Start Menu"}
	if err := storage.Write(filepath.Join(dir, "index.json"), indexer.Cache{Version: 1, Apps: []model.AppItem{item}}); err != nil {
		t.Fatal(err)
	}
	item.Source = "Manual"
	if err := storage.Write(filepath.Join(dir, "application-library.json"), applicationLibrary{Version: 1, Applications: []model.AppItem{item}}); err != nil {
		t.Fatal(err)
	}
	s := quickService(t, dir)
	if got := s.Search("Same App"); len(got) != 1 || got[0].Source != "Manual" {
		t.Fatalf("duplicate result: %+v", got)
	}
	if err := s.DeleteApplication(item.ID); err != nil {
		t.Fatal(err)
	}
	if got := s.Search("Same App"); len(got) != 1 || got[0].Source != "Start Menu" {
		t.Fatal("removing custom registration damaged scanned entry")
	}
}
