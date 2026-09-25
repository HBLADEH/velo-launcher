package platform

import (
	"context"
	"github.com/go-ole/go-ole/oleutil"
	"golang.org/x/sys/windows"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"velo-launcher/internal/config"
	"velo-launcher/internal/indexer"
	"velo-launcher/internal/search"
)

func TestDesktopRootsUseWindowsKnownFolders(t *testing.T) {
	roots := Roots(config.Defaults())
	for _, id := range []*windows.KNOWNFOLDERID{windows.FOLDERID_Desktop, windows.FOLDERID_PublicDesktop} {
		path, err := windows.KnownFolderPath(id, 0)
		if err != nil {
			t.Fatal(err)
		}
		found := false
		for _, root := range roots {
			if root.Source == "Desktop" && strings.EqualFold(root.Path, path) {
				found = true
			}
		}
		if !found {
			t.Fatalf("actual Windows desktop not scanned: %s", path)
		}
	}
}

// Read-only regression against the user's examples; never launches either game.
func TestDesktopShortcutExamples(t *testing.T) {
	if os.Getenv("VELO_INTEGRATION") != "1" {
		t.Skip("set VELO_INTEGRATION=1 to scan the real desktop examples")
	}
	r, err := NewResolver()
	if err != nil {
		t.Fatal(err)
	}
	defer r.Close()
	var roots []indexer.Root
	for _, root := range Roots(config.Defaults()) {
		if root.Source == "Desktop" || root.Source == "Start Menu" {
			roots = append(roots, root)
		}
	}
	cache, warnings, err := indexer.Scan(context.Background(), roots, indexer.Cache{}, r)
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, item := range cache.Apps {
		if item.Name == "渔力全开" {
			t.Fatal("Steam URL shortcut indexed")
		}
		if item.Name == "异环" {
			found = true
			if !strings.EqualFold(filepath.Base(item.ExecPath), "NTELauncher.exe") || !strings.EqualFold(filepath.Ext(item.Path), ".lnk") {
				t.Fatalf("unexpected target: %+v", item)
			}
			t.Logf("indexed desktop shortcut: %s -> %s", item.Path, item.ExecPath)
		}
	}
	if !found {
		t.Fatalf("异环 desktop shortcut not found; warnings=%v", warnings)
	}
	results := search.New(cache.Apps).Query("异环", 8, false, nil, 1)
	if len(results) == 0 || results[0].Name != "异环" || results[0].Source != "Desktop" {
		t.Fatalf("desktop entry not searchable: %+v", results)
	}
	t.Logf("indexed %d desktop/start menu entries; desktop name retained and Steam URL excluded", len(cache.Apps))
}

func TestPackagedAppIcon(t *testing.T) {
	if os.Getenv("VELO_INTEGRATION") != "1" {
		t.Skip("set VELO_INTEGRATION=1 to inspect installed packaged apps")
	}
	r, err := NewResolver()
	if err != nil {
		t.Fatal(err)
	}
	defer r.Close()
	apps, err := WindowsApps(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(apps) == 0 {
		t.Skip("no packaged apps installed")
	}
	for _, app := range apps {
		if _, err := ExtractIcon(app); err != nil {
			t.Errorf("%s: %v", app.Name, err)
		}
	}
}

func TestShortcutMetadataAndIcon(t *testing.T) {
	r, err := NewResolver()
	if err != nil {
		t.Fatal(err)
	}
	defer r.Close()
	path := filepath.Join(t.TempDir(), "Velo fixture.lnk")
	target := filepath.Join(os.Getenv("WINDIR"), "System32", "notepad.exe")
	v, err := oleutil.CallMethod(r.shell, "CreateShortcut", path)
	if err != nil {
		t.Fatal(err)
	}
	defer v.Clear()
	shortcut := v.ToIDispatch()
	for property, value := range map[string]string{"TargetPath": target, "Arguments": "test.txt", "WorkingDirectory": filepath.Dir(path), "Description": "Velo test", "IconLocation": target + ",0"} {
		value, err := oleutil.PutProperty(shortcut, property, value)
		if err != nil {
			t.Fatal(err)
		}
		value.Clear()
	}
	saved, err := oleutil.CallMethod(shortcut, "Save")
	if err != nil {
		t.Fatal(err)
	}
	saved.Clear()
	item, err := r.Resolve(path)
	if err != nil {
		t.Fatal(err)
	}
	item.Path = path
	if !strings.EqualFold(item.ExecPath, target) || item.Arguments != "test.txt" || item.Description != "Velo test" || !strings.EqualFold(item.WorkingDirectory, filepath.Dir(path)) || item.IconPath == "" {
		t.Fatalf("metadata: %+v", item)
	}
	img, err := ExtractIcon(item)
	if err != nil {
		t.Fatal(err)
	}
	// 索引中的图标按 iconSize 提取，前端再缩放到展示尺寸。
	if img.Bounds().Dx() != iconSize || img.Bounds().Dy() != iconSize {
		t.Fatal("unexpected icon dimensions", img.Bounds())
	}
	opaque := false
	for y := 0; y < iconSize; y++ {
		for x := 0; x < iconSize; x++ {
			_, _, _, a := img.At(x, y).RGBA()
			if a > 0 {
				opaque = true
			}
		}
	}
	if !opaque {
		t.Fatal("empty transparent icon")
	}
}
