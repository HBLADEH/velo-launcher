package platform

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"golang.org/x/sys/windows"

	"velo-launcher/internal/model"
)

func TestItemActionsUseShortcutTarget(t *testing.T) {
	for _, tt := range []struct {
		name string
		item model.AppItem
		want ItemActions
	}{
		{"shortcut", model.AppItem{Path: `C:\Desktop\#0 MuMu安卓设备.lnk`, ExecPath: `F:\MuMu Player\nx_main\MuMuNxMain.exe`, Arguments: "-v 0"}, ItemActions{true, true}},
		{"executable", model.AppItem{Path: `C:\Apps\APP.EXE`}, ItemActions{true, true}},
		{"console", model.AppItem{Path: `C:\Windows\System32\diskmgmt.msc`}, ItemActions{true, false}},
		{"unresolved shortcut", model.AppItem{Path: `C:\Desktop\App.lnk`}, ItemActions{}},
		{"package", model.AppItem{Path: `shell:AppsFolder\Package!App`, ExecPath: `shell:AppsFolder\Package!App`}, ItemActions{}},
		{"settings", model.AppItem{Path: "ms-settings:appsfeatures"}, ItemActions{}},
		{"action", model.AppItem{Path: "velo-action:shutdown"}, ItemActions{}},
		{"relative", model.AppItem{Path: "app.exe"}, ItemActions{}},
	} {
		t.Run(tt.name, func(t *testing.T) {
			if got := ActionsFor(tt.item); got != tt.want {
				t.Fatalf("got %+v, want %+v", got, tt.want)
			}
		})
	}
	item := model.AppItem{Path: `C:\Desktop\App.lnk`, ExecPath: `F:\Apps\Main.exe`, WorkingDirectory: `F:\Other`}
	if got := filepath.Dir(itemTarget(item)); got != `F:\Apps` {
		t.Fatalf("directory must come from the executable target, got %q", got)
	}
}

func TestAdminLaunchPreservesProfileAndPropagatesCancellation(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "MuMuNxMain.exe")
	if err := os.WriteFile(path, nil, 0600); err != nil {
		t.Fatal(err)
	}
	item := model.AppItem{Path: filepath.Join(dir, "#0 MuMu安卓设备.lnk"), ExecPath: path, Arguments: "-v 0", WorkingDirectory: dir}
	for _, wantErr := range []error{nil, windows.ERROR_CANCELLED} {
		called := false
		err := launchAsAdmin(item, func(verb, target, args, workingDir string) error {
			called = true
			if verb != "runas" || target != path || args != "-v 0" || workingDir != dir {
				t.Fatalf("elevation changed shortcut metadata: %q %q %q %q", verb, target, args, workingDir)
			}
			return wantErr
		})
		if !called || !errors.Is(err, wantErr) {
			t.Fatalf("called=%v, error=%v, want=%v", called, err, wantErr)
		}
	}
}

func TestInvalidItemActionsDoNotExecute(t *testing.T) {
	for _, path := range []string{"ms-settings:appsfeatures", "velo-action:shutdown", `shell:AppsFolder\Package!App`, `C:\Missing\shortcut.lnk`} {
		item := model.AppItem{Path: path}
		if OpenInstallDirectory(item) == nil || LaunchAsAdmin(item) == nil {
			t.Fatalf("unsupported target accepted: %s", path)
		}
	}
	path := filepath.Join(t.TempDir(), "missing", "app.exe")
	if OpenInstallDirectory(model.AppItem{Path: path}) == nil || LaunchAsAdmin(model.AppItem{Path: path}) == nil {
		t.Fatal("missing target accepted")
	}
}
