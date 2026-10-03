package indexer

import (
	"path/filepath"
	"testing"

	"velo-launcher/internal/model"
)

func TestVisibleBackgroundComponents(t *testing.T) {
	for _, source := range []string{"Program Files", "Program Files (x86)", "Custom"} {
		for _, name := range []string{"MuMuNxHeadless", "MuMuNxSVC", "MuMuVMMBalloonCtrl", "MuMuVMMDTrace", "MuMuVMMHeadless", "MuMuVMMManage", "MuMuVMMNetDHCP", "MuMuVMMNetNAT", "MuMuVMMSVC"} {
			t.Run(source+"/"+name, func(t *testing.T) {
				path := filepath.Join("apps", "MuMuVMMVbox", "hYpErViSoR", "tools", name+".EXE")
				items := []model.AppItem{{ID: name, Name: name, Source: source, Path: path, ExecPath: path}}
				if got := Visible(items, true); len(got) != 0 {
					t.Fatalf("background component visible: %+v", got)
				}
				if got := Visible(items, false); len(got) != 1 {
					t.Fatal("disabling noise filtering must restore the component")
				}
			})
		}
	}
}

func TestVisiblePreservesApplicationEntrypoints(t *testing.T) {
	main := filepath.Join("apps", "MuMu Player 12", "nx_main", "MuMuNxMain.exe")
	component := filepath.Join("apps", "MuMuVMMVbox", "Hypervisor", "MuMuVMMManage.exe")
	items := []model.AppItem{
		{ID: model.Identity(main, "-v 0"), Name: "#0 MuMu安卓设备", Source: "Desktop", Path: "device.lnk", ExecPath: main, Arguments: "-v 0"},
		{ID: model.Identity(main, ""), Name: "MuMu模拟器", Source: "Start Menu", Path: "mumu.lnk", ExecPath: main},
		{ID: "shortcut", Name: "VM Console", Source: "Start Menu", Path: "console.lnk", ExecPath: component},
		{ID: "custom-shortcut", Name: "VM Console Profile", Source: "Custom", Path: filepath.Join("apps", "Hypervisor", "console.lnk"), ExecPath: component, Arguments: "--profile personal"},
		{ID: "manual", Name: "MuMuVMMManage", Source: "Manual", Path: component, ExecPath: component},
		{ID: "neighbor", Name: "VirtualBox", Source: "Program Files", Path: filepath.Join("apps", "VirtualBox", "VirtualBox.exe")},
		{ID: "similar-directory", Name: "Hypervisor Manager", Source: "Program Files", Path: filepath.Join("apps", "Hypervisor Manager", "manager.exe")},
		{ID: "filename", Name: "Hypervisor", Source: "Program Files", Path: filepath.Join("apps", "Hypervisor.exe")},
	}
	if got := Visible(items, true); len(got) != len(items) {
		t.Fatalf("application entrypoints were removed: %+v", got)
	}
}
