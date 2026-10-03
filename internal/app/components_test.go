package app

import (
	"path/filepath"
	"testing"

	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/storage"
)

func TestCachedMuMuSearchFiltersComponentsAndKeepsLaunchArguments(t *testing.T) {
	dir := t.TempDir()
	main := filepath.Join(dir, "MuMuNxMain.exe")
	apps := []model.AppItem{
		{ID: model.Identity(main, "-v 0"), Name: "#0 MuMu安卓设备", Source: "Desktop", Path: filepath.Join(dir, "desktop", "device.lnk"), ExecPath: main, Arguments: "-v 0"},
		{ID: model.Identity(main, ""), Name: "MuMu模拟器", Source: "Start Menu", Path: filepath.Join(dir, "start", "mumu.lnk"), ExecPath: main},
	}
	for _, name := range []string{"MuMuNxHeadless", "MuMuVMMNetNAT", "MuMuVMMManage"} {
		path := filepath.Join(dir, "MuMuVMMVbox", "Hypervisor", name+".exe")
		apps = append(apps, model.AppItem{ID: model.Identity(path, ""), Name: name, Source: "Program Files", Path: path, ExecPath: path})
	}
	if err := storage.Write(filepath.Join(dir, "index.json"), indexer.Cache{Version: 1, Apps: apps}); err != nil {
		t.Fatal(err)
	}
	s := quickService(t, dir)
	s.onChange = func() {}
	checkEntrypoints := func() {
		t.Helper()
		for _, query := range []string{"mumu", "MuMu", "mu"} {
			got := s.Search(query)
			// Short queries can also match legitimate system tools via pinyin.
			appsOnly := got[:0]
			for _, result := range got {
				if result.Source != "System" {
					appsOnly = append(appsOnly, result)
				}
			}
			got = appsOnly
			if len(got) != 2 {
				t.Fatalf("%q should only find the two launch shortcuts: %+v", query, got)
			}
			for n, want := range apps[:2] {
				if got[n].ID != want.ID || got[n].Path != want.Path || got[n].Arguments != want.Arguments {
					t.Fatalf("shortcut launch metadata changed: %+v", got[n])
				}
			}
		}
	}
	checkEntrypoints()
	c := s.Settings()
	c.FilterNoise = false
	if err := s.SaveSettings(c); err != nil {
		t.Fatal(err)
	}
	if got := s.Search("mumu"); len(got) != len(apps) {
		t.Fatalf("disabling filtering should restore cached components: %+v", got)
	}
	// Explicitly pinned components stay available even when filtering is enabled.
	if err := s.SetPinned(apps[2].ID, true); err != nil {
		t.Fatal(err)
	}
	c.FilterNoise = true
	if err := s.SaveSettings(c); err != nil {
		t.Fatal(err)
	}
	if got := s.Search("mumu"); len(got) != 3 || got[0].ID != apps[2].ID || !got[0].Pinned {
		t.Fatalf("explicitly pinned component should survive filtering: %+v", got)
	}
	if err := s.SetPinned(apps[2].ID, false); err != nil {
		t.Fatal(err)
	}
	checkEntrypoints()
}
