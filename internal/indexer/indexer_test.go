package indexer

import (
	"context"
	"os"
	"path/filepath"
	"sort"
	"testing"
	"velo-launcher/internal/model"
)

type resolver struct{ calls int }

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
