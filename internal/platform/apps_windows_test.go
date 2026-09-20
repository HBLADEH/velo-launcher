package platform

import (
	"context"
	"github.com/go-ole/go-ole/oleutil"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

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
