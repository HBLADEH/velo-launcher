package indexer

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"testing"
	"velo-launcher/internal/model"
)

type resolver struct{ calls int }

func TestDeduplicatePrefersDesktopAndKeepsStartMenuAlias(t *testing.T) {
	items := []model.AppItem{
		{ID: "same", Name: "启动 雷电手机快取", Path: "start.lnk", Source: "Start Menu"},
		{ID: "same", Name: "雷电取证", Path: "desktop.lnk", Source: "Desktop", Keywords: []string{"forensic"}},
	}
	got := Deduplicate(items)
	if len(got) != 1 || got[0].Path != "desktop.lnk" || got[0].Name != "雷电取证" || !slices.Contains(got[0].Keywords, "启动 雷电手机快取") || !slices.Contains(got[0].Keywords, "forensic") {
		t.Fatalf("lost shortcut alias or launch path: %+v", got)
	}
}

type resolveFunc func(string) (model.AppItem, error)

func (f resolveFunc) Resolve(path string) (model.AppItem, error) { return f(path) }

func TestDesktopFileTargetShortcutsAndCacheRevalidation(t *testing.T) {
	desktop := t.TempDir()
	target := filepath.Join(t.TempDir(), "NTELauncher.exe")
	if err := os.WriteFile(target, []byte("fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"异环.lnk", "空目标.lnk", "失效.lnk", "损坏.lnk", "目录.lnk", "渔力全开.url"} {
		if err := os.WriteFile(filepath.Join(desktop, name), []byte("fixture"), 0600); err != nil {
			t.Fatal(err)
		}
	}
	calls := map[string]int{}
	r := resolveFunc(func(path string) (model.AppItem, error) {
		name := filepath.Base(path)
		calls[name]++
		switch name {
		case "异环.lnk":
			return model.AppItem{ExecPath: target, Arguments: "--profile game", WorkingDirectory: filepath.Dir(target)}, nil
		case "空目标.lnk":
			return model.AppItem{}, nil
		case "失效.lnk":
			return model.AppItem{ExecPath: filepath.Join(desktop, "missing.exe")}, nil
		case "目录.lnk":
			return model.AppItem{ExecPath: desktop}, nil
		default:
			return model.AppItem{}, fmt.Errorf("cannot resolve shortcut")
		}
	})
	roots := []Root{{Path: desktop, Source: "Desktop"}}
	cache, _, err := Scan(context.Background(), roots, Cache{}, r)
	if err != nil || len(cache.Apps) != 1 || cache.Apps[0].Name != "异环" {
		t.Fatalf("desktop discovery: %+v, %v", cache.Apps, err)
	}
	item := cache.Apps[0]
	if item.Path != filepath.Join(desktop, "异环.lnk") || item.ExecPath != target || item.Arguments != "--profile game" || item.WorkingDirectory != filepath.Dir(target) {
		t.Fatalf("launch metadata lost: %+v", item)
	}
	if calls["渔力全开.url"] != 0 {
		t.Fatal("URL shortcut resolved as a file shortcut")
	}
	cache, _, err = Scan(context.Background(), roots, cache, r)
	if err != nil || len(cache.Apps) != 1 || calls["异环.lnk"] != 1 {
		t.Fatal("valid shortcut not cached")
	}
	if err := os.Remove(target); err != nil {
		t.Fatal(err)
	}
	cache, _, err = Scan(context.Background(), roots, cache, r)
	if err != nil || len(cache.Apps) != 0 {
		t.Fatal("missing cached target still indexed")
	}
	if err := os.WriteFile(target, []byte("restored"), 0600); err != nil {
		t.Fatal(err)
	}
	cache, _, err = Scan(context.Background(), roots, cache, r)
	if err != nil || len(cache.Apps) != 1 {
		t.Fatal("restored target not rediscovered")
	}
}

func (r *resolver) Resolve(path string) (model.AppItem, error) {
	r.calls++
	return model.AppItem{ExecPath: filepath.Join(filepath.Dir(path), "code.exe")}, nil
}
func TestIncrementalDedupAndDeletion(t *testing.T) {
	dir := t.TempDir()
	for _, name := range []string{"Code.lnk", "code.exe", "ignored.txt"} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte("one"), 0600); err != nil {
			t.Fatal(err)
		}
	}
	r := &resolver{}
	roots := []Root{{Path: dir, Source: "Start Menu"}}
	cache, _, err := Scan(context.Background(), roots, Cache{}, r)
	if err != nil {
		t.Fatal(err)
	}
	if len(cache.Apps) != 1 || cache.Apps[0].Name != "Code" || r.calls != 1 {
		t.Fatalf("dedup: %+v calls=%d", cache.Apps, r.calls)
	}
	cache, _, err = Scan(context.Background(), roots, cache, r)
	if err != nil || r.calls != 1 {
		t.Fatal("unchanged link resolved again")
	}
	if err := os.WriteFile(filepath.Join(dir, "Code.lnk"), []byte("changed file"), 0600); err != nil {
		t.Fatal(err)
	}
	cache, _, err = Scan(context.Background(), roots, cache, r)
	if err != nil || r.calls != 2 {
		t.Fatal("changed link not resolved")
	}
	if err := os.Remove(filepath.Join(dir, "Code.lnk")); err != nil {
		t.Fatal(err)
	}
	cache, _, err = Scan(context.Background(), roots, cache, r)
	if err != nil || len(cache.Apps) != 1 || cache.Apps[0].Name != "code" {
		t.Fatal("deleted shortcut not removed")
	}
}
func TestCancelledScan(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, _, err := Scan(ctx, []Root{{Path: t.TempDir(), Source: "Custom"}}, Cache{}, &resolver{}); err == nil {
		t.Fatal("cancellation ignored")
	}
}

func TestScanDepthLimitSkipsBundledTools(t *testing.T) {
	root := t.TempDir()
	for _, relative := range []string{
		`App\App.exe`,
		`Vendor\Product\Product.exe`,
		`Git\usr\bin\sh.exe`,
		`Vendor\Product\bin\helper.exe`,
	} {
		path := filepath.Join(root, relative)
		if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte("fixture"), 0600); err != nil {
			t.Fatal(err)
		}
	}
	cache, _, err := Scan(context.Background(), []Root{{Path: root, Source: "Program Files", MaxDepth: 2}}, Cache{}, &resolver{})
	if err != nil {
		t.Fatal(err)
	}
	names := []string{}
	for _, item := range cache.Apps {
		names = append(names, filepath.Base(item.Path))
	}
	sort.Strings(names)
	if len(names) != 2 || names[0] != "App.exe" || names[1] != "Product.exe" {
		t.Fatalf("深层组件应被跳过: %v", names)
	}
	// MaxDepth 为 0 时恢复完整扫描。
	cache, _, err = Scan(context.Background(), []Root{{Path: root, Source: "Program Files"}}, Cache{}, &resolver{})
	if err != nil || len(cache.Apps) != 4 {
		t.Fatalf("无限制时应索引全部可执行文件: %d %v", len(cache.Apps), err)
	}
}
